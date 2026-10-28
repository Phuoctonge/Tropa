use super::warp_failure::{FailureReason, FailureStage, WarpFailure};
use super::warp_timing::{Phase, PhaseTimings};
use crate::{
    config::{
        warp_endpoint::WarpEndpoint,
        warp_profiles::{TransportKind, WarpProfile, WarpTransport},
    },
    core::{
        warp_probe::ProbeCancellation,
        warp_runtime::{RuntimeAccount, RuntimeIdentity},
    },
    utils::help::save_yaml_str,
};
use anyhow::{Result, anyhow, ensure};
use sha2::{Digest, Sha256};
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};

const SCOUT_HASH: &str = "01f11965da6415b08a23c334ea055edbda7298d46f717c2065ca94c538fb730c";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScoutPreset {
    WireGuard,
    AwgStandard,
    AwgQuic,
    AwgStun,
    AwgSip,
    AwgDns,
    AwgRandom,
    AwgNoInit,
    MasqueH3,
    MasqueH2,
    MasqueH3Sni(&'static str),
    MasqueH2Sni(&'static str),
}

impl ScoutPreset {
    pub(crate) fn parse(name: &str) -> Result<Self> {
        Ok(match name {
            "wg" => Self::WireGuard,
            "standard" => Self::AwgStandard,
            "quic" => Self::AwgQuic,
            "stun" => Self::AwgStun,
            "sip" => Self::AwgSip,
            "dns" => Self::AwgDns,
            "random" => Self::AwgRandom,
            "no-init" => Self::AwgNoInit,
            "masque" => Self::MasqueH3,
            "masque-h2" => Self::MasqueH2,
            "masque-apple" => Self::MasqueH3Sni("www.apple.com"),
            "masque-google" => Self::MasqueH3Sni("www.google.com"),
            "masque-microsoft" => Self::MasqueH3Sni("www.microsoft.com"),
            "masque-jsdelivr" => Self::MasqueH3Sni("cdn.jsdelivr.net"),
            "masque-h2-apple" => Self::MasqueH2Sni("www.apple.com"),
            "masque-h2-google" => Self::MasqueH2Sni("www.google.com"),
            "masque-h2-microsoft" => Self::MasqueH2Sni("www.microsoft.com"),
            "masque-h2-jsdelivr" => Self::MasqueH2Sni("cdn.jsdelivr.net"),
            _ => return Err(WarpFailure::new(FailureStage::Scanner, FailureReason::Unsupported).into()),
        })
    }

    pub(crate) fn kind(self) -> TransportKind {
        match self {
            Self::WireGuard => TransportKind::WireGuard,
            Self::MasqueH3 | Self::MasqueH3Sni(_) => TransportKind::MasqueH3,
            Self::MasqueH2 | Self::MasqueH2Sni(_) => TransportKind::MasqueH2,
            _ => TransportKind::Awg,
        }
    }

    fn protocol_args(self, policy: &crate::config::warp_selection::SelectionPolicy, ipv6: bool) -> Vec<String> {
        let protocol = match self {
            Self::WireGuard => "wg",
            Self::MasqueH3 | Self::MasqueH3Sni(_) => "masque",
            Self::MasqueH2 | Self::MasqueH2Sni(_) => "masque-h2",
            _ => "awg",
        };
        let mut args = vec!["-proto".into(), protocol.into()];
        if ipv6 {
            args.push("-6".into());
        }
        match self.kind() {
            TransportKind::WireGuard | TransportKind::Awg => args.extend(policy.scanner_args()),
            // WarpScout rejects geography flags for MASQUE; enforce them on fresh runtime metadata instead.
            TransportKind::MasqueH3 | TransportKind::MasqueH2 => {}
        }
        let init = match self {
            Self::AwgQuic => Some("quic"),
            Self::AwgStun => Some("stun"),
            Self::AwgSip => Some("sip"),
            Self::AwgDns => Some("dns"),
            Self::AwgRandom => Some("random"),
            _ => None,
        };
        if let Some(init) = init {
            args.extend(["-gen-i1", init].map(str::to_owned));
            if init != "random" {
                args.extend(["-i1-sni", "www.google.com"].map(str::to_owned));
            }
        }
        if matches!(self, Self::AwgRandom | Self::AwgNoInit) {
            args.push("-gen-junk".into());
        }
        if self == Self::AwgNoInit {
            args.extend(["-i1", "none"].map(str::to_owned));
        }
        if let Self::MasqueH3Sni(sni) | Self::MasqueH2Sni(sni) = self {
            args.extend(["-masque-sni", sni].map(str::to_owned));
        }
        args
    }

    fn validate_export(self, profile: &WarpProfile) -> Result<()> {
        let matches = profile.transport.kind() == self.kind()
            && match (self, &profile.transport) {
                (Self::MasqueH3Sni(expected), WarpTransport::MasqueH3 { sni })
                | (Self::MasqueH2Sni(expected), WarpTransport::MasqueH2 { sni }) => sni == expected,
                (Self::AwgNoInit, WarpTransport::Awg { i1, .. }) => i1.is_empty(),
                (Self::AwgRandom, WarpTransport::Awg { i1, .. }) => !i1.is_empty(),
                _ => true,
            };
        ensure!(
            matches,
            WarpFailure::new(FailureStage::Scanner, FailureReason::InvalidOutput)
        );
        Ok(())
    }

    fn seed_profile(self) -> Result<WarpProfile> {
        let sni = match self {
            Self::MasqueH3Sni(sni) | Self::MasqueH2Sni(sni) => sni,
            _ => "consumer-masque.cloudflareclient.com",
        }
        .to_owned();
        let transport = match self.kind() {
            TransportKind::WireGuard => WarpTransport::WireGuard,
            TransportKind::Awg => WarpTransport::Awg {
                jc: 4,
                jmin: 40,
                jmax: 70,
                i1: if self == Self::AwgNoInit {
                    String::new()
                } else {
                    "<r 32>".into()
                },
            },
            TransportKind::MasqueH3 => WarpTransport::MasqueH3 { sni },
            TransportKind::MasqueH2 => WarpTransport::MasqueH2 { sni },
        };
        let endpoint = match self.kind() {
            TransportKind::MasqueH3 | TransportKind::MasqueH2 => "162.159.198.1:443",
            _ => "188.114.98.58:2408",
        };
        WarpProfile::new(endpoint.parse()?, transport)
    }
}

struct ScannerResources<T> {
    paths: [std::path::PathBuf; 3],
    _execution: std::sync::Arc<T>,
}

impl<T> Drop for ScannerResources<T> {
    fn drop(&mut self) {
        // Remove credentials before releasing the sidecar slot, after the child has exited.
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub(crate) struct ScoutCandidate {
    pub(crate) profile: WarpProfile,
    pub(crate) node: Option<String>,
    pub(crate) node_location: Option<String>,
    pub(crate) endpoint_ping_ms: Option<u32>,
    pub(crate) tunnel_ping_ms: Option<u32>,
    pub(crate) tunnel_loss_percent: Option<u8>,
}

fn parse_scout_report(report: &str, template: &WarpProfile) -> Result<Vec<ScoutCandidate>> {
    ensure!(report.len() <= 256 * 1024, "WarpScout report exceeded its limit");
    let mut started = false;
    let mut tunnel_measured = false;
    let mut seen = std::collections::HashSet::new();
    let mut candidates = Vec::new();
    for line in report.lines() {
        if line.starts_with("ENDPOINT ") {
            if started {
                break;
            }
            started = true;
            tunnel_measured = line.contains("TUN PING");
            continue;
        }
        if !started {
            continue;
        }
        if line.starts_with('#') {
            break;
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.is_empty() {
            continue;
        }
        ensure!(fields.len() >= 4, "Invalid WarpScout report row");
        let endpoint: std::net::SocketAddr = fields[0].parse().map_err(|_| anyhow!("Invalid WarpScout endpoint"))?;
        ensure!(
            endpoint.is_ipv6() == template.endpoint.is_ipv6(),
            "WarpScout report mixes address families"
        );
        let profile = WarpProfile::new(endpoint, template.transport.clone())?;
        let node_index = fields.iter().enumerate().skip(2).find_map(|(index, field)| {
            (field.len() == 3 && field.bytes().all(|b| b.is_ascii_uppercase())).then_some(index)
        });
        let node = node_index.map(|index| fields[index].to_string());
        let node_location = node_index.and_then(|index| {
            let location = fields[index + 1..].join(" ");
            (!location.is_empty()
                && location != "?"
                && location.len() <= 120
                && !location.chars().any(char::is_control))
            .then_some(location)
        });
        let latency = |text: &str| {
            text.strip_suffix("ms")
                .and_then(|ms| ms.parse::<f64>().ok())
                .filter(|ms| ms.is_finite() && *ms > 0.0 && *ms <= 60_000.0)
                .map(|ms| ms.ceil() as u32)
        };
        let endpoint_ping_ms = latency(fields[1]);
        let tunnel_ping_ms = tunnel_measured.then(|| latency(fields[2])).flatten();
        let tunnel_loss_percent = tunnel_measured
            .then(|| {
                fields[3]
                    .strip_suffix('%')
                    .and_then(|loss| loss.parse::<u8>().ok())
                    .filter(|loss| *loss <= 100)
            })
            .flatten();
        if seen.insert(endpoint) {
            candidates.push(ScoutCandidate {
                profile,
                node,
                node_location,
                endpoint_ping_ms,
                tunnel_ping_ms,
                tunnel_loss_percent,
            });
        }
        if candidates.len() >= 128 {
            break;
        }
    }
    ensure!(
        started,
        WarpFailure::new(FailureStage::Scanner, FailureReason::InvalidOutput)
    );
    ensure!(
        !candidates.is_empty(),
        WarpFailure::new(FailureStage::Scanner, FailureReason::NoCandidates)
    );
    Ok(candidates)
}

#[cfg(test)]
#[test]
fn report_candidates_exclude_torn_rows_and_keep_distinct_nodes() -> anyhow::Result<()> {
    let template = WarpProfile::new(
        WarpEndpoint::parse("8.35.211.1:500")?.address(),
        WarpTransport::WireGuard,
    )?;
    let report = "# WARP endpoints: 4 working / 5 probed\n\nENDPOINT ENDPOINT PING SEEN AS NODE NODE LOCATION\n8.35.211.1:500 8ms RU HEL Helsinki, FI\n8.35.211.2:500 9ms RU HEL Helsinki, FI\n8.35.211.3:2408 10ms RU FRA Frankfurt-am-Main, DE\n188.114.96.2:500 11ms RU FRA Frankfurt-am-Main, DE\n\n# 1 torn down\nENDPOINT ENDPOINT PING SEEN AS NODE NODE LOCATION\n8.6.112.1:500 1ms RU LED St. Petersburg, RU\n# Best endpoint per node\n";
    let candidates = parse_scout_report(report, &template)?;
    assert_eq!(candidates.len(), 4);
    assert_eq!(candidates[2].node.as_deref(), Some("FRA"));
    assert!(
        candidates
            .iter()
            .all(|candidate| !candidate.profile.endpoint.ip().to_string().starts_with("8.6.112."))
    );
    assert!(
        parse_scout_report(
            "ENDPOINT ENDPOINT PING SEEN AS NODE NODE LOCATION\n127.0.0.1:500 1ms RU HEL Helsinki, FI",
            &template
        )
        .is_err()
    );
    Ok(())
}

#[cfg(test)]
#[test]
fn report_keeps_all_seventy_working_addresses() -> anyhow::Result<()> {
    let template = WarpProfile::new(
        WarpEndpoint::parse("8.35.211.1:500")?.address(),
        WarpTransport::WireGuard,
    )?;
    let mut report = String::from("ENDPOINT ENDPOINT PING TUN PING LOSS SEEN AS NODE NODE LOCATION\n");
    for number in 1..=70 {
        report.push_str(&format!("8.35.211.{number}:500 3ms 5ms 0% RU DME Moscow, RU\n"));
    }
    let candidates = parse_scout_report(&report, &template)?;
    assert_eq!(candidates.len(), 70);
    assert_eq!(candidates[0].endpoint_ping_ms, Some(3));
    assert_eq!(candidates[0].tunnel_ping_ms, Some(5));
    assert_eq!(candidates[0].tunnel_loss_percent, Some(0));
    Ok(())
}

fn parse_scout_profile(bytes: &[u8], endpoint: &str, awg: bool) -> Result<WarpProfile> {
    ensure!(bytes.len() <= 64 * 1024, "WarpScout export exceeded its limit");
    let export: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| anyhow!("Invalid WarpScout profile export"))?;
    let proxies = export.as_array().ok_or_else(|| anyhow!("Missing WarpScout proxy"))?;
    ensure!(proxies.len() == 1, "Unexpected nested WarpScout configuration");
    let proxy = &proxies[0];
    ensure!(
        proxy["type"].as_str() == Some("wireguard"),
        "Unexpected WarpScout transport"
    );
    let peers = proxy["peers"]
        .as_array()
        .ok_or_else(|| anyhow!("Missing WarpScout peer"))?;
    ensure!(peers.len() == 1, "Unexpected WarpScout peer count");
    let peer = &peers[0];
    let address = WarpEndpoint::parse(endpoint)?.address();
    ensure!(
        peer["server"]
            .as_str()
            .and_then(|server| server.parse::<std::net::IpAddr>().ok())
            == Some(address.ip())
            && peer["port"].as_u64() == Some(u64::from(address.port())),
        "WarpScout export does not match its selected endpoint"
    );
    let transport = if awg {
        let options = &proxy["amnezia-wg-option"];
        for (key, value) in [("s1", 0), ("s2", 0), ("h1", 1), ("h2", 2), ("h3", 3), ("h4", 4)] {
            ensure!(
                options[key].as_u64() == Some(value),
                "Unsupported AWG header parameters"
            );
        }
        let number = |key: &str| -> Result<u16> {
            options[key]
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or_else(|| anyhow!("Missing AWG junk parameters"))
        };
        WarpTransport::Awg {
            jc: number("jc")?,
            jmin: number("jmin")?,
            jmax: number("jmax")?,
            i1: match options.get("i1") {
                None => String::new(),
                Some(value) => value
                    .as_str()
                    .ok_or_else(|| anyhow!("Invalid AWG init parameters"))?
                    .into(),
            },
        }
    } else {
        ensure!(
            proxy.get("amnezia-wg-option").is_none(),
            "Unexpected AWG parameters in WireGuard export"
        );
        WarpTransport::WireGuard
    };
    WarpProfile::new(address, transport)
}

fn parse_scout_profile_for_transport(bytes: &[u8], endpoint: &str, kind: TransportKind) -> Result<WarpProfile> {
    match kind {
        TransportKind::WireGuard | TransportKind::Awg => {
            return parse_scout_profile(bytes, endpoint, kind == TransportKind::Awg);
        }
        TransportKind::MasqueH3 | TransportKind::MasqueH2 => {}
    }
    ensure!(bytes.len() <= 64 * 1024, "WarpScout export exceeded its limit");
    let export: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| anyhow!("Invalid WarpScout profile export"))?;
    let proxies = export.as_array().ok_or_else(|| anyhow!("Missing WarpScout proxy"))?;
    ensure!(proxies.len() == 1, "Unexpected nested WarpScout configuration");
    let proxy = &proxies[0];
    ensure!(
        proxy["type"].as_str() == Some("masque"),
        "Unexpected WarpScout transport"
    );
    ensure!(
        proxy.get("peers").is_none() && proxy.get("amnezia-wg-option").is_none(),
        "Unexpected nested WarpScout transport"
    );
    let address: std::net::SocketAddr = endpoint.parse().map_err(|_| anyhow!("Invalid WarpScout endpoint"))?;
    ensure!(
        proxy["server"]
            .as_str()
            .and_then(|server| server.parse::<std::net::IpAddr>().ok())
            == Some(address.ip())
            && proxy["port"].as_u64() == Some(u64::from(address.port())),
        "WarpScout export does not match its selected endpoint"
    );
    for field in ["skip-cert-verify", "h3-l4proxy"] {
        ensure!(
            proxy.get(field).is_none() || proxy[field].as_bool() == Some(false),
            "Unsupported MASQUE security parameters"
        );
    }
    ensure!(
        proxy.get("uri").is_none() || proxy["uri"].as_str() == Some("https://cloudflareaccess.com"),
        "Unsupported MASQUE connect URI"
    );
    let sni = proxy["sni"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing MASQUE SNI"))?
        .to_owned();
    let transport = match kind {
        TransportKind::MasqueH3 => {
            ensure!(proxy.get("network").is_none(), "Unexpected MASQUE network selector");
            WarpTransport::MasqueH3 { sni }
        }
        TransportKind::MasqueH2 => {
            ensure!(
                proxy["network"].as_str() == Some("h2"),
                "Missing MASQUE HTTP/2 selector"
            );
            WarpTransport::MasqueH2 { sni }
        }
        _ => unreachable!(),
    };
    WarpProfile::new(address, transport)
}

fn scan_failure(stderr: &[u8]) -> WarpFailure {
    let diagnostic = String::from_utf8_lossy(stderr);
    WarpFailure::new(
        FailureStage::Scanner,
        if diagnostic.contains("no working endpoints found")
            || diagnostic.contains("no MASQUE endpoint passed data")
        {
            FailureReason::NoCandidates
        } else {
            FailureReason::ProcessExited
        },
    )
}

#[cfg(test)]
#[test]
fn scanner_failure_classification_never_returns_raw_diagnostics() {
    assert_eq!(
        scan_failure(b"no working endpoints found - try -p awg").reason,
        FailureReason::NoCandidates
    );
    assert_eq!(
        scan_failure(b"no MASQUE endpoint passed data - this network blocks it, try -p awg").reason,
        FailureReason::NoCandidates
    );
    let failed = scan_failure(b"synthetic-sensitive-marker: unexpected failure");
    assert_eq!(failed.reason, FailureReason::ProcessExited);
    assert!(!failed.to_string().contains("synthetic-sensitive-marker"));
}

#[cfg(test)]
#[test]
fn advanced_presets_use_scan_flags_and_keep_masque_transport_and_sni() -> anyhow::Result<()> {
    let policy = Default::default();
    for (name, expected) in [("random", "-gen-i1"), ("no-init", "-i1")] {
        let preset = ScoutPreset::parse(name)?;
        let args = preset.protocol_args(&policy, false);
        assert!(args.iter().any(|arg| arg == "-gen-junk"));
        assert!(
            args.windows(2)
                .any(|pair| pair == [expected, if name == "random" { "random" } else { "none" }])
        );
        assert_eq!(preset.kind(), TransportKind::Awg);
        preset.seed_profile()?.validate()?;
    }
    for (name, protocol, sni) in [
        ("masque-apple", "masque", "www.apple.com"),
        ("masque-h2-google", "masque-h2", "www.google.com"),
        ("masque-microsoft", "masque", "www.microsoft.com"),
        ("masque-h2-jsdelivr", "masque-h2", "cdn.jsdelivr.net"),
    ] {
        let preset = ScoutPreset::parse(name)?;
        let args = preset.protocol_args(&policy, true);
        assert!(args.windows(2).any(|pair| pair == ["-proto", protocol]));
        assert!(args.windows(2).any(|pair| pair == ["-masque-sni", sni]));
        assert!(args.iter().any(|arg| arg == "-6"));
        assert!(
            matches!(preset.seed_profile()?.transport, WarpTransport::MasqueH3 { sni: value } | WarpTransport::MasqueH2 { sni: value } if value == sni)
        );
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn awg_export_without_optional_i1_is_distinct_from_malformed_i1() -> anyhow::Result<()> {
    let mut export = serde_json::json!([{
        "type":"wireguard", "peers":[{"server":"188.114.98.58","port":2408}],
        "amnezia-wg-option":{"jc":5,"jmin":20,"jmax":80,"s1":0,"s2":0,"h1":1,"h2":2,"h3":3,"h4":4}
    }]);
    let omitted = parse_scout_profile(&serde_json::to_vec(&export)?, "188.114.98.58:2408", true)?;
    assert!(matches!(&omitted.transport, WarpTransport::Awg { i1, .. } if i1.is_empty()));
    export[0]["amnezia-wg-option"]["i1"] = "".into();
    let explicit = parse_scout_profile(&serde_json::to_vec(&export)?, "188.114.98.58:2408", true)?;
    assert_eq!(omitted, explicit);
    assert_eq!(
        serde_json::from_str::<WarpProfile>(&serde_json::to_string(&explicit)?)?,
        explicit
    );
    for invalid in [
        serde_json::Value::Null,
        serde_json::json!(3),
        serde_json::json!("none"),
        serde_json::json!("<r 999999>"),
    ] {
        export[0]["amnezia-wg-option"]["i1"] = invalid;
        assert!(parse_scout_profile(&serde_json::to_vec(&export)?, "188.114.98.58:2408", true).is_err());
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn scanner_export_preserves_generated_awg_parameters_without_credentials() -> anyhow::Result<()> {
    let bytes = br#"[{
        "type":"wireguard", "private-key":"synthetic-secret",
        "peers":[{"server":"8.39.204.212","port":1701,"public-key":"synthetic-peer-secret"}],
        "amnezia-wg-option":{"jc":6,"jmin":10,"jmax":50,"s1":0,"s2":0,"h1":1,"h2":2,"h3":3,"h4":4,"i1":"<b 0x0102><r 32><b 0xabcd><r 16>"}
    }]"#;
    let candidate = parse_scout_profile(bytes, "8.39.204.212:1701", true)?;
    assert!(!serde_json::to_string(&candidate)?.contains("synthetic-secret"));
    assert!(
        matches!(candidate.transport, crate::config::warp_profiles::WarpTransport::Awg { i1, .. } if i1 == "<b 0x0102><r 32><b 0xabcd><r 16>")
    );
    assert!(parse_scout_profile(bytes, "8.39.204.213:1701", true).is_err());
    Ok(())
}

pub(crate) async fn scan(
    root: &std::path::Path,
    scout: &std::path::Path,
    cancellation: &ProbeCancellation,
    preset: &str,
    deadline: tokio::time::Instant,
    multiple: bool,
    sample: u8,
    ipv6: bool,
    policy: &crate::config::warp_selection::SelectionPolicy,
    timings: &PhaseTimings,
) -> Result<Vec<ScoutCandidate>> {
    let preset = ScoutPreset::parse(preset)?;
    let clock = timings.clock();
    ensure!(
        !cancellation.is_cancelled(),
        WarpFailure::new(FailureStage::Scanner, FailureReason::Cancelled)
    );
    let bytes = tokio::fs::read(scout)
        .await
        .map_err(|_| anyhow!("Unable to read pinned WarpScout"))?;
    let hash: String = Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect();
    ensure!(hash == SCOUT_HASH, "WarpScout hash differs from pinned v0.16.1");
    let _execution = std::sync::Arc::new(
        crate::core::service::reserve_sidecar()
            .await
            .map_err(|_| anyhow!("Another core/service is active or ownership could not be reserved"))?,
    );
    let seed = preset.seed_profile()?;
    let (account, _) = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(WarpFailure::new(FailureStage::Registration, FailureReason::Cancelled)
            .for_transport(preset.kind()).into()),
        _ = tokio::time::sleep_until(deadline) => return Err(WarpFailure::new(FailureStage::Registration, FailureReason::Timeout)
            .for_transport(preset.kind()).into()),
        result = RuntimeAccount::ensure(root, &seed) => result.map_err(|error| {
            WarpFailure::from_error(&error, FailureStage::Registration, FailureReason::InvalidOutput)
                .for_transport(preset.kind())
        })?,
    };
    let exported_account = match account.identity() {
        RuntimeIdentity::WireGuard(account) => account.scout_account(),
        RuntimeIdentity::Masque(account) => account.scout_account(),
    };
    let path = root.join("secrets/warpscout-scan-account.json");
    let export_path = root.join("secrets/warpscout-profile-export.json");
    let report_path = root.join("secrets/warpscout-scan-report.txt");
    let _cleanup_phase = scopeguard::guard((), |_| clock.switch(Phase::Cleanup));
    let scanner_owner = std::sync::Arc::new(ScannerResources {
        paths: [path.clone(), export_path.clone(), report_path.clone()],
        _execution: std::sync::Arc::clone(&_execution),
    });
    save_yaml_str(&path, &exported_account.to_string())
        .await
        .map_err(|_| anyhow!("Unable to export temporary scanner account"))?;
    let mut command = Command::new(scout);
    command
        .args([
            "scan",
            "-best",
            "-plain",
            "-sample",
            if sample == 1 { "1" } else { "5" },
            "-tunnel-jobs",
            "12",
            "-timeout",
            "2",
            "-account",
        ])
        .arg(&path)
        .arg("-conf")
        .arg(&export_path)
        .args(["-conf-type", "mihomo-json"])
        .current_dir(root)
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.args(preset.protocol_args(policy, ipv6));
    if multiple {
        command.arg("-o").arg(&report_path).arg("-P");
    } else {
        command.arg("-no-report");
    }
    ensure!(
        !cancellation.is_cancelled(),
        WarpFailure::new(FailureStage::Scanner, FailureReason::Cancelled)
    );
    ensure!(
        tokio::time::Instant::now() < deadline,
        WarpFailure::new(FailureStage::Scanner, FailureReason::Timeout)
    );
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let mut child = command
        .spawn()
        .map_err(|error| WarpFailure::from_error(&error.into(), FailureStage::Scanner, FailureReason::ProcessExited))?;
    #[cfg(windows)]
    crate::core::warp_children::track(
        child.id().expect("spawned scanner PID"),
        std::sync::Arc::clone(&scanner_owner),
    );
    #[cfg(windows)]
    let job = match child
        .id()
        .and_then(|pid| crate::core::manager::create_and_assign_sidecar_job(pid).ok())
    {
        Some(job) => job,
        None => {
            clock.switch(Phase::Cleanup);
            let _ = child.start_kill();
            ensure!(
                matches!(
                    tokio::time::timeout(Duration::from_secs(5), child.wait()).await,
                    Ok(Ok(_))
                ),
                WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending)
            );
            return Err(anyhow!("Unable to protect WarpScout with a Windows Job Object"));
        }
    };
    let output = child.stdout.take().expect("piped scanner output");
    let reader = tokio::spawn(async move {
        let mut bytes = Vec::new();
        output.take(4097).read_to_end(&mut bytes).await.map(|_| bytes)
    });
    let errors = child.stderr.take().expect("piped scanner diagnostics");
    let diagnostics = tokio::spawn(async move {
        let mut bytes = Vec::new();
        let _ = errors.take(64 * 1024).read_to_end(&mut bytes).await;
        bytes
    });
    clock.switch(Phase::Scan);
    let mut timed_out = false;
    let status = tokio::select! {
        status = child.wait() => status.map_err(|_| anyhow!("Unable to wait for WarpScout")),
        _ = tokio::time::sleep_until(deadline) => {
            timed_out = true;
            Err(anyhow!("WarpScout scan timed out"))
        },
        _ = cancellation.cancelled() => Err(anyhow!("WarpScout scan cancelled")),
    };
    clock.switch(Phase::Cleanup);
    let _ = child.start_kill();
    let stopped = matches!(
        tokio::time::timeout(Duration::from_secs(5), child.wait()).await,
        Ok(Ok(_))
    );
    #[cfg(windows)]
    drop(job);
    ensure!(
        stopped,
        WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending)
    );
    #[cfg(windows)]
    crate::core::warp_children::wait_for_exit().await?;
    let exported = async {
        let file = tokio::fs::File::open(&export_path).await?;
        let mut bytes = Vec::new();
        file.take(64 * 1024 + 1).read_to_end(&mut bytes).await?;
        Ok::<_, std::io::Error>(bytes)
    }
    .await;
    let export_removed = match tokio::fs::remove_file(&export_path).await {
        Ok(()) => true,
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    };
    let removed = tokio::fs::remove_file(&path).await.is_ok();
    ensure!(
        removed,
        WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending)
    );
    ensure!(
        export_removed,
        WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending)
    );
    ensure!(
        !cancellation.is_cancelled(),
        WarpFailure::new(FailureStage::Scanner, FailureReason::Cancelled)
    );
    clock.switch(Phase::Preparation);
    if timed_out {
        return Err(WarpFailure::new(FailureStage::Scanner, FailureReason::Timeout).into());
    }
    if !status?.success() {
        let diagnostics = tokio::time::timeout(Duration::from_secs(1), diagnostics)
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default();
        if String::from_utf8_lossy(&diagnostics).contains("no working endpoints found") {
            return Ok(Vec::new());
        }
        return Err(scan_failure(&diagnostics).into());
    }
    let bytes = tokio::time::timeout(Duration::from_secs(1), reader)
        .await
        .map_err(|_| anyhow!(WarpFailure::new(FailureStage::Scanner, FailureReason::Timeout)))?
        .map_err(|_| anyhow!("WarpScout output reader failed"))?
        .map_err(|_| anyhow!("Unable to read WarpScout output"))?;
    let endpoint = std::str::from_utf8(&bytes)
        .map_err(|_| WarpFailure::new(FailureStage::Scanner, FailureReason::InvalidOutput))?;
    let profile = parse_scout_profile_for_transport(
        &exported.map_err(|_| anyhow!("WarpScout profile export is missing"))?,
        endpoint.trim(),
        preset.kind(),
    )
    .map_err(|error| WarpFailure::from_error(&error, FailureStage::Scanner, FailureReason::InvalidOutput))?;
    ensure!(
        profile.endpoint.is_ipv6() == ipv6,
        "WarpScout returned the wrong address family"
    );
    preset.validate_export(&profile)?;
    if multiple {
        let file = tokio::fs::File::open(&report_path)
            .await
            .map_err(|_| anyhow!("WarpScout report is missing"))?;
        let mut report = Vec::new();
        file.take(256 * 1024 + 1).read_to_end(&mut report).await?;
        clock.switch(Phase::Cleanup);
        tokio::fs::remove_file(&report_path)
            .await
            .map_err(|_| anyhow!(WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending)))?;
        clock.switch(Phase::Preparation);
        parse_scout_report(
            std::str::from_utf8(&report).map_err(|_| anyhow!("Invalid WarpScout report encoding"))?,
            &profile,
        )
    } else {
        Ok(vec![ScoutCandidate {
            profile,
            node: None,
            node_location: None,
            endpoint_ping_ms: None,
            tunnel_ping_ms: None,
            tunnel_loss_percent: None,
        }])
    }
}

#[cfg(test)]
#[test]
fn empty_scanner_table_is_distinct_from_a_missing_or_malformed_report() -> anyhow::Result<()> {
    let template = WarpProfile::new(
        WarpEndpoint::parse("8.35.211.1:500")?.address(),
        WarpTransport::WireGuard,
    )?;
    let empty = parse_scout_report("ENDPOINT ENDPOINT PING SEEN AS NODE NODE LOCATION\n", &template)
        .err()
        .expect("empty table cannot provide a candidate");
    let missing = parse_scout_report("synthetic-sensitive-marker", &template)
        .err()
        .expect("missing table is invalid output");
    assert_ne!(empty.to_string(), missing.to_string());
    assert!(!missing.to_string().contains("synthetic-sensitive-marker"));
    Ok(())
}

#[cfg(test)]
#[test]
fn masque_report_uses_its_own_endpoint_pool_and_preserves_measurements() -> anyhow::Result<()> {
    let template = WarpProfile::new(
        "162.159.198.1:443".parse()?,
        WarpTransport::MasqueH3 {
            sni: "consumer-masque.cloudflareclient.com".into(),
        },
    )?;
    let candidates = parse_scout_report(
        "ENDPOINT ENDPOINT PING TUN PING LOSS SEEN AS NODE NODE LOCATION\n162.159.198.2:8443 - 12.3ms 0% DE FRA Frankfurt, DE\n",
        &template,
    )?;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].profile.endpoint.to_string(), "162.159.198.2:8443");
    assert_eq!(candidates[0].profile.transport, template.transport);
    assert_eq!(candidates[0].endpoint_ping_ms, None);
    assert_eq!(candidates[0].tunnel_ping_ms, Some(13));
    assert_eq!(candidates[0].tunnel_loss_percent, Some(0));
    assert!(parse_scout_report(
        "ENDPOINT ENDPOINT PING TUN PING LOSS SEEN AS NODE NODE LOCATION\n8.35.211.1:500 - 12ms 0% DE FRA Frankfurt, DE\n", &template
    ).is_err());
    Ok(())
}

#[cfg(test)]
#[test]
fn masque_export_requires_requested_mode_and_discards_credentials() -> anyhow::Result<()> {
    use crate::config::warp_profiles::TransportKind;
    let h3 = serde_json::json!([{
        "type":"masque", "server":"162.159.198.1", "port":443,
        "sni":"consumer-masque.cloudflareclient.com",
        "private-key":"synthetic-private", "public-key":"synthetic-peer",
        "ip":"172.16.0.2", "udp":true
    }]);
    let parse = |value: &serde_json::Value, endpoint, kind| {
        parse_scout_profile_for_transport(&serde_json::to_vec(value).unwrap(), endpoint, kind)
    };
    let profile = parse(&h3, "162.159.198.1:443", TransportKind::MasqueH3)?;
    assert!(matches!(profile.transport, WarpTransport::MasqueH3 { .. }));
    let serialized = serde_json::to_string(&profile)?;
    assert!(!serialized.contains("synthetic-private"));
    assert!(!serialized.contains("synthetic-peer"));
    assert!(parse(&h3, "162.159.198.2:443", TransportKind::MasqueH3).is_err());
    assert!(parse(&h3, "162.159.198.1:443", TransportKind::MasqueH2).is_err());
    let mut h2 = h3.clone();
    h2[0]["network"] = "h2".into();
    h2[0]["server"] = "162.159.199.20".into();
    let profile = parse(&h2, "162.159.199.20:443", TransportKind::MasqueH2)?;
    assert!(matches!(profile.transport, WarpTransport::MasqueH2 { .. }));
    assert!(parse(&h2, "162.159.199.20:443", TransportKind::MasqueH3).is_err());
    for (field, value) in [
        ("skip-cert-verify", serde_json::json!(true)),
        ("h3-l4proxy", serde_json::json!(true)),
        ("sni", serde_json::json!("bad sni")),
        ("network", serde_json::json!("unknown")),
        ("uri", serde_json::json!("https://other.example")),
    ] {
        let mut invalid = h3.clone();
        invalid[0][field] = value;
        assert!(
            parse(&invalid, "162.159.198.1:443", TransportKind::MasqueH3).is_err(),
            "accepted {field}"
        );
    }
    Ok(())
}

#[cfg(all(test, windows))]
#[tokio::test]
async fn scanner_files_and_execution_lease_survive_caller_until_child_exit() -> anyhow::Result<()> {
    let root = std::env::temp_dir().join(format!("tropa-scout-lease-{}", nanoid::nanoid!()));
    std::fs::create_dir(&root)?;
    let _root = scopeguard::guard(root.clone(), |root| {
        let _ = std::fs::remove_dir_all(root);
    });
    let paths = [
        root.join("account.json"),
        root.join("export.json"),
        root.join("report.txt"),
    ];
    for path in &paths {
        std::fs::write(path, b"synthetic-scanner-data")?;
    }
    let execution = std::sync::Arc::new(());
    let weak = std::sync::Arc::downgrade(&execution);
    let owner = std::sync::Arc::new(ScannerResources {
        paths: paths.clone(),
        _execution: execution,
    });
    let mut child = tokio::process::Command::new("ping")
        .args(["-n", "999", "127.0.0.1"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    crate::core::warp_children::track(child.id().unwrap(), std::sync::Arc::clone(&owner));
    drop(owner);
    tokio::task::yield_now().await;
    assert!(weak.upgrade().is_some());
    assert!(paths.iter().all(|path| path.exists()));
    child.kill().await?;
    crate::core::warp_children::wait_for_exit().await?;
    assert!(paths.iter().all(|path| !path.exists()));
    assert!(weak.upgrade().is_none());
    Ok(())
}

#[cfg(test)]
#[test]
fn scanner_protocol_options_are_typed_and_do_not_forward_masque_geography_filters() -> anyhow::Result<()> {
    let policy = crate::config::warp_selection::SelectionPolicy {
        include_node_countries: vec!["DE".into()],
        exclude_nodes: vec!["DME".into()],
        ..Default::default()
    };
    for (name, kind, protocol) in [
        ("wg", TransportKind::WireGuard, "wg"),
        ("standard", TransportKind::Awg, "awg"),
        ("masque", TransportKind::MasqueH3, "masque"),
        ("masque-h2", TransportKind::MasqueH2, "masque-h2"),
    ] {
        let preset = ScoutPreset::parse(name)?;
        assert_eq!(preset.kind(), kind);
        let args = preset.protocol_args(&policy, false);
        assert_eq!(&args[..2], ["-proto", protocol]);
        if kind == TransportKind::WireGuard || kind == TransportKind::Awg {
            assert!(args.windows(2).any(|pair| pair == ["-country", "DE"]));
        } else {
            assert!(
                !args
                    .iter()
                    .any(|arg| arg == "-country" || arg == "-exclude-node" || arg == "-gen-i1")
            );
        }
        assert_eq!(preset.seed_profile()?.transport.kind(), kind);
    }
    assert!(ScoutPreset::parse("typo").is_err());
    let quic = ScoutPreset::parse("quic")?.protocol_args(&policy, false);
    assert!(quic.windows(2).any(|pair| pair == ["-gen-i1", "quic"]));
    Ok(())
}

#[cfg(test)]
#[test]
fn ipv6_exports_compare_addresses_not_their_textual_spelling() -> anyhow::Result<()> {
    let wg = serde_json::to_vec(&serde_json::json!([{
        "type":"wireguard", "peers":[{"server":"2606:4700:00d0:0000:0000:0000:0000:0001", "port":2408}]
    }]))?;
    let parsed = parse_scout_profile(&wg, "[2606:4700:d0::1]:2408", false)?;
    assert_eq!(parsed.endpoint.to_string(), "[2606:4700:d0::1]:2408");
    for (kind, network) in [(TransportKind::MasqueH3, None), (TransportKind::MasqueH2, Some("h2"))] {
        let mut proxy = serde_json::json!({"type":"masque", "server":"2606:4700:0103:0:0:0:0:1", "port":443,
            "sni":"consumer-masque.cloudflareclient.com"});
        if let Some(network) = network {
            proxy["network"] = network.into();
        }
        let bytes = serde_json::to_vec(&vec![proxy])?;
        let parsed = parse_scout_profile_for_transport(&bytes, "[2606:4700:103::1]:443", kind)?;
        assert_eq!(parsed.endpoint.to_string(), "[2606:4700:103::1]:443");
        assert!(parse_scout_profile_for_transport(&bytes, "[2606:4700:103::2]:443", kind).is_err());
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn ipv6_reports_keep_brackets_and_cannot_mix_endpoint_families() -> anyhow::Result<()> {
    let template = WarpProfile::new("[2606:4700:d0::1]:2408".parse()?, WarpTransport::WireGuard)?;
    let header = "ENDPOINT ENDPOINT PING TUN PING LOSS SEEN AS NODE NODE LOCATION\n";
    let rows = parse_scout_report(
        &format!("{header}[2606:4700:d1::2]:500 3ms 5ms 0% DE FRA Frankfurt, DE\n"),
        &template,
    )?;
    assert_eq!(rows[0].profile.endpoint.to_string(), "[2606:4700:d1::2]:500");
    assert!(
        parse_scout_report(
            &format!("{header}188.114.96.1:500 3ms 5ms 0% DE FRA Frankfurt, DE\n"),
            &template
        )
        .is_err()
    );
    Ok(())
}

#[cfg(test)]
#[test]
fn ipv6_flag_is_independent_of_protocol_and_geography_constraints() -> anyhow::Result<()> {
    let policy = crate::config::warp_selection::SelectionPolicy::default();
    for name in ["wg", "standard", "quic", "masque", "masque-h2"] {
        let preset = ScoutPreset::parse(name)?;
        assert!(!preset.protocol_args(&policy, false).iter().any(|arg| arg == "-6"));
        assert_eq!(
            preset
                .protocol_args(&policy, true)
                .iter()
                .filter(|arg| *arg == "-6")
                .count(),
            1
        );
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn advanced_export_must_match_requested_sni_and_init_mode() -> Result<()> {
    for name in ["masque-apple", "masque-h2-google", "no-init", "random"] {
        let preset = ScoutPreset::parse(name)?;
        let mut profile = preset.seed_profile()?;
        if name == "random" {
            profile.transport = WarpTransport::Awg {
                jc: 6,
                jmin: 22,
                jmax: 80,
                i1: "<r 50>".into(),
            };
        }
        preset.validate_export(&profile)?;
        match &mut profile.transport {
            WarpTransport::MasqueH3 { sni } | WarpTransport::MasqueH2 { sni } => *sni = "wrong.example.com".into(),
            WarpTransport::Awg { i1, .. } => {
                *i1 = if name == "no-init" {
                    "<r 32>".into()
                } else {
                    String::new()
                }
            }
            _ => unreachable!(),
        }
        assert!(preset.validate_export(&profile).is_err(), "{name}");
    }
    Ok(())
}

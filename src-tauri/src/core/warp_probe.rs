use super::warp_failure::{FailureReason, FailureStage, WarpFailure};
#[cfg(test)]
use super::warp_runtime::RuntimeIdentity;
use super::warp_runtime::profile_probe_config;
#[cfg(test)]
use super::warp_runtime::profile_runtime_config;
#[cfg(test)]
use crate::config::warp_account::WarpAccount;
#[cfg(any(feature = "tropa-probe", test))]
use crate::config::warp_endpoint::WarpEndpoint;
#[cfg(any(feature = "tropa-probe", test))]
use crate::config::warp_profiles::WarpTransport;
use crate::config::warp_profiles::{WarpObservation, WarpProfile};

#[derive(Debug)]
pub(crate) struct TransportUnavailable;

impl std::fmt::Display for TransportUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("WARP candidate did not pass HTTPS and UDP validation")
    }
}

impl std::error::Error for TransportUnavailable {}

#[cfg(test)]
fn probe_config(account: &WarpAccount, port: u16, endpoint: WarpEndpoint) -> serde_json::Value {
    profile_probe_config(
        RuntimeIdentity::WireGuard(account),
        port,
        &WarpProfile::new(endpoint.address(), WarpTransport::WireGuard).expect("validated endpoint"),
    )
    .expect("WireGuard probe configuration")
}

fn trace_has_warp(trace: &str) -> bool {
    trace.lines().any(|line| matches!(line.trim(), "warp=on" | "warp=plus"))
}

fn valid_dns_answer(packet: &[u8], id: [u8; 2]) -> bool {
    let question = b"\x07example\x03com\x00\x00\x01\x00\x01";
    if packet.len() < 12 + question.len()
        || packet[..2] != id
        || packet[2] & 0xfa != 0x80
        || packet[3] & 0x0f != 0
        || packet[4..6] != [0, 1]
        || packet[6..8] == [0, 0]
        || packet.get(12..12 + question.len()) != Some(question.as_slice())
    {
        return false;
    }
    let mut offset = 12 + question.len();
    loop {
        let Some(&label) = packet.get(offset) else { return false };
        offset += 1;
        if label == 0 {
            break;
        }
        if label & 0xc0 == 0xc0 {
            let Some(&low) = packet.get(offset) else { return false };
            let target = (((label & 0x3f) as usize) << 8) | low as usize;
            if target >= offset - 1 {
                return false;
            }
            offset += 1;
            break;
        }
        if label > 63 {
            return false;
        }
        offset += label as usize;
    }
    let Some(header) = packet.get(offset..offset + 10) else {
        return false;
    };
    let length = u16::from_be_bytes([header[8], header[9]]) as usize;
    header[..4] == [0, 1, 0, 1] && length == 4 && packet.get(offset + 10..offset + 14).is_some()
}

#[cfg(any(feature = "tropa-probe", feature = "portable", test))]
mod live {
    use super::*;
    use crate::{core::warp_runtime::RuntimeAccount, utils::help::save_yaml_str};
    use anyhow::{Context, Result, anyhow, ensure};
    use serde::Serialize;
    use sha2::{Digest, Sha256};
    use std::{path::PathBuf, process::Stdio, time::Duration};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream, UdpSocket},
        process::Command,
    };

    const CORE_HASH: &str = "8ccf696897f4d56aa8f9b13893b6b3c6d24a8a505eb257d46eeac53e6701ed2e";

    pub(crate) struct ProbeCancellation {
        receiver: tokio::sync::watch::Receiver<bool>,
        sender: tokio::sync::watch::Sender<bool>,
        listener: tokio::task::JoinHandle<()>,
    }

    impl ProbeCancellation {
        pub(crate) fn new() -> Self {
            let (sender, receiver) = tokio::sync::watch::channel(false);
            let signal_sender = sender.clone();
            let listener = tokio::spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                let _ = signal_sender.send(true);
            });
            Self {
                receiver,
                sender,
                listener,
            }
        }

        pub(crate) fn linked(parent: &Self, deadline: tokio::time::Instant) -> Self {
            let (sender, receiver) = tokio::sync::watch::channel(parent.is_cancelled());
            let signal_sender = sender.clone();
            let mut parent_receiver = parent.receiver.clone();
            let listener = tokio::spawn(async move {
                tokio::select! {
                    _ = parent_receiver.wait_for(|cancelled| *cancelled) => {},
                    _ = tokio::time::sleep_until(deadline) => {},
                }
                let _ = signal_sender.send(true);
            });
            Self {
                receiver,
                sender,
                listener,
            }
        }

        pub(crate) fn cancel_handle(&self) -> tokio::sync::watch::Sender<bool> {
            self.sender.clone()
        }

        pub(crate) fn is_cancelled(&self) -> bool {
            *self.receiver.borrow()
        }

        pub(crate) async fn cancelled(&self) {
            let mut receiver = self.receiver.clone();
            let _ = receiver.wait_for(|cancelled| *cancelled).await;
        }
    }

    impl Drop for ProbeCancellation {
        fn drop(&mut self) {
            self.listener.abort();
        }
    }

    #[cfg(test)]
    #[tokio::test]
    async fn slice_cancellation_does_not_cancel_the_whole_search() {
        let parent = ProbeCancellation::new();
        let child = ProbeCancellation::linked(&parent, tokio::time::Instant::now());
        assert!(
            tokio::time::timeout(Duration::from_secs(1), child.cancelled())
                .await
                .is_ok()
        );
        assert!(!parent.is_cancelled());
        let next = ProbeCancellation::linked(&parent, tokio::time::Instant::now() + Duration::from_secs(60));
        parent.cancel_handle().send(true).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(1), next.cancelled())
                .await
                .is_ok()
        );
    }

    #[cfg(test)]
    #[tokio::test]
    async fn cancellation_remains_visible_between_scan_and_validation() {
        let (sender, receiver) = tokio::sync::watch::channel(false);
        let cancellation = ProbeCancellation {
            receiver,
            sender: sender.clone(),
            listener: tokio::spawn(std::future::pending()),
        };
        sender.send(true).unwrap();
        for _ in 0..2 {
            assert!(
                tokio::time::timeout(Duration::from_millis(50), cancellation.cancelled())
                    .await
                    .is_ok()
            );
            assert!(cancellation.is_cancelled());
        }
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TransportProbeReport {
        #[serde(skip)]
        pub(crate) profile: WarpProfile,
        registration_reused: bool,
        core_sha256: &'static str,
        endpoint: String,
        https_warp: bool,
        udp_dns: bool,
        core_stopped: bool,
        runtime_deleted: bool,
        pub(crate) observation: WarpObservation,
        transport: &'static str,
    }

    fn core_command(core: &std::path::Path) -> Command {
        let mut command = Command::new(core);
        command
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        command.creation_flags(0x0800_0000);
        command
    }

    async fn stop_child(child: &mut tokio::process::Child) -> bool {
        let _ = child.start_kill();
        matches!(
            tokio::time::timeout(Duration::from_secs(5), child.wait()).await,
            Ok(Ok(_))
        )
    }

    #[cfg(feature = "tropa-probe")]
    pub async fn run_warp_transport_probe(root: PathBuf, core: PathBuf) -> Result<TransportProbeReport> {
        ensure!(root.is_absolute() && core.is_absolute(), "Probe paths must be absolute");
        let _write = crate::config::Config::lock_config_write().await;
        let _catalog_write = crate::config::warp_profiles::WarpCatalogLock::acquire(&root)?;
        let cancellation = ProbeCancellation::new();
        let endpoint = WarpEndpoint::load(&root.join("warp-endpoint.json"))
            .await?
            .unwrap_or(WarpEndpoint::parse("162.159.192.1:2408")?);
        run_warp_transport_probe_at(root, core, endpoint, &cancellation).await
    }

    #[cfg(feature = "tropa-probe")]
    pub(crate) async fn run_warp_transport_probe_at(
        root: PathBuf,
        core: PathBuf,
        endpoint: WarpEndpoint,
        cancellation: &ProbeCancellation,
    ) -> Result<TransportProbeReport> {
        let profile = WarpProfile::new(endpoint.address(), WarpTransport::WireGuard)?;
        run_warp_transport_probe_profile(root, core, &profile, cancellation).await
    }

    #[cfg(feature = "tropa-probe")]
    pub(crate) async fn run_warp_transport_probe_profile(
        root: PathBuf,
        core: PathBuf,
        profile: &WarpProfile,
        cancellation: &ProbeCancellation,
    ) -> Result<TransportProbeReport> {
        run_warp_transport_probe_profile_timed(
            root,
            core,
            profile,
            cancellation,
            &crate::core::warp_timing::PhaseTimings::default(),
        )
        .await
    }

    pub(crate) async fn run_warp_transport_probe_profile_timed(
        root: PathBuf,
        core: PathBuf,
        profile: &WarpProfile,
        cancellation: &ProbeCancellation,
        timings: &crate::core::warp_timing::PhaseTimings,
    ) -> Result<TransportProbeReport> {
        use crate::core::warp_timing::Phase;
        let network_before = crate::config::warp_context::capture();
        let clock = timings.clock();
        ensure!(root.is_absolute() && core.is_absolute(), "Probe paths must be absolute");
        ensure!(
            !cancellation.is_cancelled(),
            WarpFailure::new(FailureStage::Validation, FailureReason::Cancelled)
        );
        let bytes = tokio::fs::read(&core).await.map_err(|error| {
            WarpFailure::from_error(&error.into(), FailureStage::Runtime, FailureReason::InvalidOutput)
        })?;
        ensure!(
            Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                == CORE_HASH,
            WarpFailure::new(FailureStage::Runtime, FailureReason::InvalidOutput)
        );
        let _execution = std::sync::Arc::new(
            crate::core::service::reserve_sidecar()
                .await
                .map_err(|_| anyhow!("Another core/service is active or core ownership could not be reserved"))?,
        );
        let (account, reused) = tokio::select! {
            result = RuntimeAccount::ensure_with_route(&root, profile, std::sync::Arc::new(
                crate::core::warp_registration_http::RegistrationRoute::for_runtime(
                    core.parent().and_then(std::path::Path::parent).ok_or_else(|| anyhow!("Runtime directory is missing"))?
                )
            )) => result.map_err(|error| {
                WarpFailure::from_error(&error, FailureStage::Registration, FailureReason::InvalidOutput)
                    .for_transport(profile.transport.kind())
            })?,
            _ = cancellation.cancelled() => return Err(WarpFailure::new(FailureStage::Validation, FailureReason::Cancelled)
                .for_transport(profile.transport.kind()).into()),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let path = root.join("secrets/warp-probe-runtime.json");
        let json = serde_json::to_string(&profile_probe_config(account.identity(), port, profile)?)?;
        save_yaml_str(&path, &json)
            .await
            .map_err(|_| anyhow!("Unable to save secret transport configuration"))?;
        let _cleanup_phase = scopeguard::guard((), |_| clock.switch(Phase::Cleanup));
        // Keep the file until OS-confirmed exit and remove it before releasing execution ownership.
        let runtime_owner = std::sync::Arc::new((
            scopeguard::guard(path.clone(), |path| {
                let _ = std::fs::remove_file(path);
            }),
            std::sync::Arc::clone(&_execution),
        ));
        clock.switch(Phase::Validation);
        let mut validator = core_command(&core)
            .arg("-t")
            .arg("-d")
            .arg(&root)
            .arg("-f")
            .arg(&path)
            .spawn()
            .map_err(|_| anyhow!(WarpFailure::new(FailureStage::Runtime, FailureReason::ProcessExited)))?;
        #[cfg(windows)]
        crate::core::warp_children::track(
            validator.id().expect("spawned validator PID"),
            std::sync::Arc::clone(&runtime_owner),
        );
        let validation = tokio::time::timeout(Duration::from_secs(10), async {
            tokio::select! {
                status = validator.wait() => status,
                _ = cancellation.cancelled() => Err(std::io::Error::from(std::io::ErrorKind::Interrupted)),
            }
        })
        .await;
        if !matches!(validation, Ok(Ok(_))) {
            clock.switch(Phase::Cleanup);
            ensure!(
                stop_child(&mut validator).await,
                WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending)
            );
        }
        ensure!(
            !cancellation.is_cancelled(),
            WarpFailure::new(FailureStage::Validation, FailureReason::Cancelled)
        );
        ensure!(
            matches!(validation, Ok(Ok(status)) if status.success()),
            WarpFailure::new(
                FailureStage::Runtime,
                if validation.is_err() {
                    FailureReason::Timeout
                } else {
                    FailureReason::InvalidOutput
                }
            )
        );
        let mut child = core_command(&core)
            .arg("-d")
            .arg(&root)
            .arg("-f")
            .arg(&path)
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| {
                WarpFailure::from_error(&error.into(), FailureStage::Runtime, FailureReason::ProcessExited)
            })?;
        #[cfg(windows)]
        crate::core::warp_children::track(
            child.id().expect("spawned probe PID"),
            std::sync::Arc::clone(&runtime_owner),
        );
        let output = child.stdout.take().expect("piped core output");
        let diagnostics = tokio::spawn(async move {
            let mut bytes = Vec::new();
            let _ = output.take(64 * 1024).read_to_end(&mut bytes).await;
            bytes
        });
        #[cfg(windows)]
        let job = match child
            .id()
            .and_then(|pid| crate::core::manager::create_and_assign_sidecar_job(pid).ok())
        {
            Some(job) => job,
            None => {
                clock.switch(Phase::Cleanup);
                ensure!(
                    stop_child(&mut child).await,
                    WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending)
                );
                return Err(anyhow!(WarpFailure::new(
                    FailureStage::Runtime,
                    FailureReason::InvalidOutput
                )));
            }
        };
        let validation_work = async {
            for attempt in 0..50 {
                ensure!(
                    child.try_wait()?.is_none(),
                    WarpFailure::new(FailureStage::Runtime, FailureReason::ProcessExited)
                );
                if TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).await.is_ok() {
                    break;
                }
                ensure!(
                    attempt < 49,
                    WarpFailure::new(FailureStage::Runtime, FailureReason::Timeout)
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let client = reqwest::Client::builder()
                .no_proxy()
                .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))?)
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(20))
                .build()?;
            let mut response = client
                .get("https://www.cloudflare.com/cdn-cgi/trace")
                .send()
                .await
                .map_err(|error| {
                    anyhow!(
                        "HTTPS through WARP failed ({})",
                        if error.is_timeout() {
                            "timeout"
                        } else if error.is_connect() {
                            "connection"
                        } else {
                            "request"
                        }
                    )
                })?;
            ensure!(response.status().is_success(), "HTTPS trace rejected the request");
            let mut trace = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| anyhow!("HTTPS trace could not be read"))?
            {
                ensure!(
                    trace.len() + chunk.len() <= 16 * 1024,
                    "HTTPS trace response exceeded its limit"
                );
                trace.extend_from_slice(&chunk);
            }
            ensure!(
                trace_has_warp(std::str::from_utf8(&trace).unwrap_or_default()),
                "HTTPS trace did not confirm WARP"
            );
            ensure!(
                child.try_wait()?.is_none(),
                WarpFailure::new(FailureStage::Runtime, FailureReason::ProcessExited)
            );
            let country = std::str::from_utf8(&trace)
                .unwrap_or_default()
                .lines()
                .find_map(|line| line.strip_prefix("loc="))
                .filter(|country| country.len() == 2 && country.bytes().all(|b| b.is_ascii_uppercase()))
                .map(str::to_owned);
            let metadata = super::super::warp_metadata::fetch_metadata(&client).await.ok();
            Ok::<_, anyhow::Error>((country, metadata))
        };
        let checked = tokio::select! {
            result = validation_work => result,
            _ = cancellation.cancelled() => Err(anyhow!(WarpFailure::new(FailureStage::Validation, FailureReason::Cancelled))),
        };
        let udp_checked = async {
            let mut timings = Vec::new();
            for _ in 0..5 {
                let sample = tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => return Err(anyhow!(WarpFailure::new(FailureStage::Validation, FailureReason::Cancelled))),
                    result = tokio::time::timeout(Duration::from_secs(3), udp_dns(port)) => result,
                };
                if let Ok(Ok(timing)) = sample {
                    timings.push(timing);
                }
            }
            ensure!(
                timings.len() >= 3,
                "Fewer than three UDP DNS checks passed through WARP"
            );
            Ok(timings)
        }
        .await;
        clock.switch(Phase::Cleanup);
        let stopped = stop_child(&mut child).await;
        #[cfg(windows)]
        drop(job);
        ensure!(
            stopped,
            WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending)
        );
        #[cfg(windows)]
        crate::core::warp_children::wait_for_exit().await?;
        let removed = tokio::fs::remove_file(&path).await.is_ok();
        ensure!(
            removed,
            WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending)
        );
        ensure!(
            !cancellation.is_cancelled(),
            WarpFailure::new(FailureStage::Validation, FailureReason::Cancelled)
        );
        clock.switch(Phase::Preparation);
        let diagnostics = tokio::time::timeout(Duration::from_secs(1), diagnostics)
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default();
        if let Err(error) = &checked {
            if let Some(failure) = error.downcast_ref::<WarpFailure>() {
                return Err(failure.for_transport(profile.transport.kind()).into());
            }
            let log = String::from_utf8_lossy(&diagnostics).to_ascii_lowercase();
            return Err(anyhow!(TransportUnavailable)).context(format!(
                "{error}; UDP DNS confirmed={}; core diagnostics: handshake={}, timeout={}, dns={}, network_unreachable={}",
                udp_checked.is_ok(),
                log.contains("handshake"),
                log.contains("timeout") || log.contains("deadline exceeded"),
                log.contains("dns"),
                log.contains("network is unreachable")
            ));
        }
        let timings = udp_checked.map_err(|_| anyhow!(TransportUnavailable))?;
        let jitter_ms = if timings.len() > 1 {
            timings.windows(2).map(|pair| pair[0].abs_diff(pair[1])).sum::<u32>() / (timings.len() - 1) as u32
        } else {
            0
        };
        let mut sorted = timings.clone();
        sorted.sort_unstable();
        let (exit_country, metadata) = checked?;
        let observation = WarpObservation {
            runtime_method: Some(crate::config::warp_profiles::RuntimeMeasurementMethod::UdpDnsV1),
            network_context: network_before
                .filter(|before| crate::config::warp_context::capture().as_ref() == Some(before)),
            scanner_context: None,
            checked_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            https_warp: true,
            samples: 5,
            failed: 5 - timings.len() as u8,
            median_ms: sorted[sorted.len() / 2],
            jitter_ms,
            exit_country,
            node_country: metadata.as_ref().and_then(|meta| meta.country.clone()),
            node: metadata.and_then(|meta| meta.node),
            node_location: None,
            endpoint_ping_ms: None,
            tunnel_ping_ms: None,
            tunnel_loss_percent: None,
            scanner_checked_at: None,
            search_measurement: None,
        };
        Ok(TransportProbeReport {
            profile: profile.clone(),
            registration_reused: reused,
            core_sha256: CORE_HASH,
            endpoint: profile.endpoint.to_string(),
            https_warp: true,
            udp_dns: true,
            core_stopped: stopped,
            runtime_deleted: removed,
            observation,
            transport: profile.transport.label(),
        })
    }

    async fn udp_dns(port: u16) -> Result<u32> {
        let mut control = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).await?;
        control.write_all(&[5, 1, 0]).await?;
        let mut auth = [0u8; 2];
        control.read_exact(&mut auth).await?;
        ensure!(auth == [5, 0], "Unexpected SOCKS authentication reply");
        let udp = UdpSocket::bind("127.0.0.1:0").await?;
        let mut associate = vec![5, 3, 0, 1, 127, 0, 0, 1];
        associate.extend_from_slice(&udp.local_addr()?.port().to_be_bytes());
        control.write_all(&associate).await?;
        let mut header = [0u8; 4];
        control.read_exact(&mut header).await?;
        ensure!(header == [5, 0, 0, 1], "Unexpected SOCKS UDP reply");
        let mut address = [0u8; 6];
        control.read_exact(&mut address).await?;
        let relay_ip = std::net::Ipv4Addr::new(address[0], address[1], address[2], address[3]);
        ensure!(
            relay_ip.is_loopback() || relay_ip.is_unspecified(),
            "SOCKS UDP relay is not local"
        );
        let relay_port = u16::from_be_bytes([address[4], address[5]]);
        let mut id = [0u8; 2];
        getrandom::fill(&mut id).map_err(|_| anyhow!("Unable to generate DNS query ID"))?;
        let mut packet = vec![
            0, 0, 0, 1, 1, 1, 1, 1, 0, 53, id[0], id[1], 1, 0, 0, 1, 0, 0, 0, 0, 0, 0,
        ];
        packet.extend_from_slice(b"\x07example\x03com\x00\x00\x01\x00\x01");
        let started = std::time::Instant::now();
        udp.send_to(&packet, (std::net::Ipv4Addr::LOCALHOST, relay_port))
            .await?;
        let mut response = [0u8; 4096];
        let (size, sender) = udp.recv_from(&mut response).await?;
        ensure!(
            sender.ip().is_loopback() && sender.port() == relay_port,
            "Unexpected SOCKS UDP sender"
        );
        ensure!(
            size >= 22 && response[..10] == [0, 0, 0, 1, 1, 1, 1, 1, 0, 53],
            "Unexpected SOCKS UDP packet"
        );
        ensure!(
            valid_dns_answer(&response[10..size], id),
            "DNS response did not match the query"
        );
        Ok(started.elapsed().as_millis().clamp(1, u32::MAX as u128) as u32)
    }
}

#[cfg(feature = "tropa-probe")]
pub use live::run_warp_transport_probe;
#[cfg(any(feature = "tropa-probe", feature = "portable", test))]
pub(crate) use live::{ProbeCancellation, TransportProbeReport, run_warp_transport_probe_profile_timed};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::warp_account::WarpAccount;
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    #[tokio::test]
    async fn external_cancellation_survives_listener_drop() {
        let cancellation = ProbeCancellation::new();
        let sender = cancellation.cancel_handle();
        sender.send_replace(true);
        assert!(cancellation.is_cancelled());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), cancellation.cancelled())
                .await
                .is_ok()
        );
    }

    #[test]
    fn persistent_runtime_starts_without_routes_and_keeps_dns_inside_warp() -> anyhow::Result<()> {
        let account = WarpAccount::from_warpscout_json(&serde_json::to_vec(&serde_json::json!({
            "id":"synthetic-device", "token":"synthetic-token",
            "private_key":STANDARD.encode([7u8;32]), "peer_public_key":STANDARD.encode([9u8;32]),
            "ipv4":"172.16.0.2","ipv6":null
        }))?)?;
        let profile = WarpProfile::new("188.114.96.10:500".parse()?, WarpTransport::WireGuard)?;
        let config = profile_runtime_config(RuntimeIdentity::WireGuard(&account), &profile, &Default::default())?;
        assert_eq!(config["tun"]["enable"], false);
        assert_eq!(config["tun"]["auto-route"], true);
        assert_eq!(config["tun"]["strict-route"], true);
        assert_eq!(config["ipv6"], true);
        assert_eq!(
            config["dns"]["nameserver"],
            serde_json::json!(["https://1.1.1.1/dns-query#WARP"])
        );
        assert_eq!(config["rules"], serde_json::json!(["MATCH,WARP"]));
        assert_eq!(config["mixed-port"], 0);
        assert_eq!(config["log-level"], "error");
        assert!(!config.to_string().contains("synthetic-"));
        let routing = crate::config::tropa::RoutingPreferences {
            mode: crate::config::tropa::RoutingMode::Selected,
            applications: vec![r"C:\Apps\Firefox\firefox.exe".into()],
        };
        let selected = profile_runtime_config(RuntimeIdentity::WireGuard(&account), &profile, &routing)?;
        assert_eq!(selected["rules"], serde_json::to_value(routing.rules()?)?);
        assert_eq!(selected["find-process-mode"], "always");
        assert_eq!(
            selected["dns"]["nameserver"],
            serde_json::json!(["https://1.1.1.1/dns-query#DIRECT"])
        );
        assert!(
            profile_runtime_config(
                RuntimeIdentity::WireGuard(&account),
                &profile,
                &crate::config::tropa::RoutingPreferences {
                    mode: crate::config::tropa::RoutingMode::Selected,
                    applications: vec![]
                }
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn probe_config_is_loopback_only_and_carries_no_api_identity() -> anyhow::Result<()> {
        let account = WarpAccount::from_warpscout_json(&serde_json::to_vec(&serde_json::json!({
            "id":"synthetic-device-marker", "token":"synthetic-token-marker",
            "private_key":STANDARD.encode([7u8;32]), "peer_public_key":STANDARD.encode([9u8;32]),
            "ipv4":"172.16.0.2","ipv6":null
        }))?)?;
        let config = probe_config(&account, 12345, WarpEndpoint::parse("188.114.96.10:500")?);
        assert_eq!(config["proxies"][0]["server"], "188.114.96.10");
        assert_eq!(config["proxies"][0]["port"], 500);
        let transport = crate::config::warp_profiles::WarpTransport::Awg {
            jc: 6,
            jmin: 10,
            jmax: 50,
            i1: "<b 0x0102abcd>".into(),
        };
        let profile = crate::config::warp_profiles::WarpProfile::new("8.39.204.212:1701".parse()?, transport)?;
        let awg_config = profile_probe_config(RuntimeIdentity::WireGuard(&account), 12345, &profile)?;
        assert_eq!(
            awg_config["proxies"][0]["amnezia-wg-option"],
            serde_json::json!({
                "jc":6,"jmin":10,"jmax":50,"s1":0,"s2":0,"h1":1,"h2":2,"h3":3,"h4":4,"i1":"<b 0x0102abcd>"
            })
        );
        assert_eq!(awg_config["proxies"][0]["port"], 1701);
        assert_eq!(config["tun"]["enable"], false);
        assert_eq!(config["bind-address"], "127.0.0.1");
        assert_eq!(config["allow-lan"], false);
        assert_eq!(config["proxies"][0]["private-key"], STANDARD.encode([7u8; 32]));
        assert_eq!(config["proxies"][0]["public-key"], STANDARD.encode([9u8; 32]));
        assert_eq!(config["proxies"][0]["ip"], "172.16.0.2");
        assert!(!config.to_string().contains("-marker"));
        assert!(!config.as_object().unwrap().contains_key("external-controller"));
        assert!(trace_has_warp("ip=192.0.2.1\nwarp=on\n"));
        assert!(!trace_has_warp("warp=off\n"));
        let mut dns = vec![0u8; 12];
        dns[..2].copy_from_slice(&[1, 2]);
        dns[2] = 0x81;
        dns[3] = 0x80;
        dns[5] = 1;
        dns[7] = 1;
        assert!(!valid_dns_answer(&dns, [1, 2]));
        dns.extend_from_slice(b"\x07example\x03com\x00\x00\x01\x00\x01");
        dns.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4, 192, 0, 2, 1]);
        assert!(valid_dns_answer(&dns, [1, 2]));
        assert!(!valid_dns_answer(&dns[..dns.len() - 1], [1, 2]));
        assert!(!valid_dns_answer(&dns, [1, 3]));
        dns[3] = 0x83;
        assert!(!valid_dns_answer(&dns, [1, 2]));
        Ok(())
    }
}

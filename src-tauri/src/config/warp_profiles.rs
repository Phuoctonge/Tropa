use anyhow::{Result, anyhow, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{cmp::Ordering, collections::HashSet, net::SocketAddr, path::Path};
use tokio::io::AsyncReadExt;

use super::warp_endpoint::WarpEndpoint;

const MAX_BYTES: usize = 1024 * 1024;
const MAX_PROFILES: usize = 128;
const MAX_SCANNED_PROFILES: usize = 256;

pub(crate) struct WarpCatalogLock {
    _file: std::fs::File,
}

impl WarpCatalogLock {
    pub(crate) fn acquire(root: &Path) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("warp-profiles.lock"))
            .map_err(|_| anyhow!("Unable to open WARP catalog lock"))?;
        file.try_lock()
            .map_err(|_| anyhow!("WARP profile discovery or selection is already in progress"))?;
        Ok(Self { _file: file })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistrationFamily {
    WireGuard,
    Masque,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TransportKind {
    WireGuard,
    Awg,
    MasqueH3,
    MasqueH2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum WarpTransport {
    WireGuard,
    Awg { jc: u16, jmin: u16, jmax: u16, i1: String },
    MasqueH3 { sni: String },
    MasqueH2 { sni: String },
}

impl WarpTransport {
    pub(crate) fn family(&self) -> RegistrationFamily {
        match self {
            Self::WireGuard | Self::Awg { .. } => RegistrationFamily::WireGuard,
            Self::MasqueH3 { .. } | Self::MasqueH2 { .. } => RegistrationFamily::Masque,
        }
    }

    pub(crate) fn kind(&self) -> TransportKind {
        match self {
            Self::WireGuard => TransportKind::WireGuard,
            Self::Awg { .. } => TransportKind::Awg,
            Self::MasqueH3 { .. } => TransportKind::MasqueH3,
            Self::MasqueH2 { .. } => TransportKind::MasqueH2,
        }
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::WireGuard => "WireGuard",
            Self::Awg { .. } => "AWG",
            Self::MasqueH3 { .. } => "MASQUE HTTP/3",
            Self::MasqueH2 { .. } => "MASQUE HTTP/2",
        }
    }

    fn validate(&self, endpoint: SocketAddr) -> Result<()> {
        match self {
            Self::WireGuard | Self::Awg { .. } => {
                WarpEndpoint::parse(&endpoint.to_string())?;
                if let Self::Awg { jc, jmin, jmax, i1 } = self {
                    ensure!(
                        *jc <= 128 && (*jc > 0 || !i1.is_empty()) && jmin <= jmax && *jmax <= 1280,
                        "Invalid AWG junk parameters"
                    );
                    ensure!(i1.is_empty() || valid_init_chain(i1), "Invalid AWG init parameters");
                }
            }
            Self::MasqueH3 { sni } | Self::MasqueH2 { sni } => {
                let allowed = match endpoint {
                    SocketAddr::V4(address) => {
                        let octets = address.ip().octets();
                        match self {
                            Self::MasqueH3 { .. } => octets == [162, 159, 198, 1] || octets == [162, 159, 198, 2],
                            _ => octets[..3] == [162, 159, 198] || octets[..3] == [162, 159, 199],
                        }
                    }
                    SocketAddr::V6(address) => {
                        let segments = address.ip().segments();
                        let pool = address.scope_id() == 0
                            && address.flowinfo() == 0
                            && segments[0] == 0x2606
                            && segments[1] == 0x4700
                            && matches!(segments[2], 0x103 | 0x104);
                        pool && (!matches!(self, Self::MasqueH3 { .. })
                            || (segments[3..7] == [0; 4] && matches!(segments[7], 1 | 2)))
                    }
                };
                ensure!(
                    allowed && matches!(endpoint.port(), 443 | 500 | 1701 | 4500 | 4443 | 8443 | 8095),
                    "Invalid MASQUE endpoint"
                );
                ensure!(
                    sni.len() <= 253
                        && sni.contains('.')
                        && sni.split('.').all(|label| !label.is_empty()
                            && label.len() <= 63
                            && !label.starts_with('-')
                            && !label.ends_with('-')
                            && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')),
                    "Invalid MASQUE SNI"
                );
            }
        }
        Ok(())
    }
}

fn valid_init_chain(mut chain: &str) -> bool {
    if chain.is_empty() || chain.len() > 8192 {
        return false;
    }
    let mut total = 0usize;
    while !chain.is_empty() {
        let Some((tag, rest)) = chain.strip_prefix('<').and_then(|s| s.split_once('>')) else {
            return false;
        };
        let Some((kind, value)) = tag.split_once(' ') else {
            return false;
        };
        let length = match kind {
            "b" => {
                let Some(hex) = value.strip_prefix("0x") else {
                    return false;
                };
                if hex.is_empty() || hex.len() % 2 != 0 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return false;
                }
                hex.len() / 2
            }
            "r" | "rc" | "rd" => match value.parse::<usize>() {
                Ok(n) if n > 0 && n <= 1280 => n,
                _ => return false,
            },
            _ => return false,
        };
        total += length;
        if total > 1280 {
            return false;
        }
        chain = rest;
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WarpProfile {
    pub(crate) id: String,
    pub(crate) endpoint: SocketAddr,
    pub(crate) transport: WarpTransport,
}

impl WarpProfile {
    pub(crate) fn new(endpoint: SocketAddr, transport: WarpTransport) -> Result<Self> {
        transport.validate(endpoint)?;
        let bytes = serde_json::to_vec(&(endpoint, &transport))?;
        let id = Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(Self {
            id,
            endpoint,
            transport,
        })
    }

    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            Self::new(self.endpoint, self.transport.clone())?.id == self.id,
            "WARP profile identity does not match its parameters"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SearchMeasurement {
    pub(crate) engine_build_id: String,
    pub(crate) stages: SearchStages,
    pub(crate) endpoint_ping_ms: Option<f64>,
    pub(crate) tunnel_ping_ms: Option<f64>,
    pub(crate) loss_percent: Option<f64>,
    pub(crate) jitter_ms: Option<f64>,
    pub(crate) samples: u32,
    pub(crate) method: Option<String>,
    pub(crate) exit_country: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SearchStages {
    pub(crate) handshake: Option<bool>,
    pub(crate) http: Option<bool>,
    pub(crate) https: Option<bool>,
    pub(crate) udp: Option<bool>,
}

impl SearchMeasurement {
    fn validate(&self) -> Result<()> {
        let snapshot = self.engine_build_id.strip_prefix("source:").unwrap_or_default();
        ensure!(snapshot.len() == 64 && snapshot.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()), "Invalid search engine build");
        ensure!(
            [self.endpoint_ping_ms, self.tunnel_ping_ms, self.jitter_ms]
                .into_iter().flatten().all(|ms| ms.is_finite() && ms >= 0.0)
                && self.loss_percent.is_none_or(|loss| loss.is_finite() && (0.0..=100.0).contains(&loss)),
            "Invalid search measurement"
        );
        ensure!(self.method.as_ref().is_none_or(|method| !method.is_empty() && method.len() <= 128 && method.bytes().all(|b| b.is_ascii_graphic())), "Invalid search measurement method");
        ensure!(
            (self.tunnel_ping_ms.is_none() && self.loss_percent.is_none() && self.jitter_ms.is_none())
                || (self.samples > 0 && self.method.is_some()),
            "Search measurement lacks sampling provenance"
        );
        ensure!(
            ![self.stages.http, self.stages.https, self.stages.udp].contains(&Some(true))
                || self.stages.handshake == Some(true),
            "Search data stage lacks handshake"
        );
        ensure!(self.exit_country.as_ref().is_none_or(|country| country.len() == 2 && country.bytes().all(|b| b.is_ascii_uppercase())), "Invalid search exit country");
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WarpObservation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) network_context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scanner_context: Option<String>,
    pub(crate) checked_at: u64,
    pub(crate) https_warp: bool,
    pub(crate) samples: u8,
    pub(crate) failed: u8,
    pub(crate) median_ms: u32,
    pub(crate) jitter_ms: u32,
    #[serde(alias = "country")]
    pub(crate) exit_country: Option<String>,
    #[serde(default)]
    pub(crate) node_country: Option<String>,
    #[serde(default)]
    pub(crate) node: Option<String>,
    #[serde(default)]
    pub(crate) node_location: Option<String>,
    #[serde(default)]
    pub(crate) endpoint_ping_ms: Option<u32>,
    #[serde(default)]
    pub(crate) tunnel_ping_ms: Option<u32>,
    #[serde(default)]
    pub(crate) tunnel_loss_percent: Option<u8>,
    #[serde(default)]
    pub(crate) scanner_checked_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) search_measurement: Option<SearchMeasurement>,
}

impl WarpObservation {
    fn validate(&self) -> Result<()> {
        if let Some(measurement) = &self.search_measurement {
            measurement.validate()?;
        }
        ensure!(
            super::warp_context::valid(self.network_context.as_deref())
                && super::warp_context::valid(self.scanner_context.as_deref()),
            "Invalid measurement context"
        );
        ensure!(
            self.scanner_checked_at.is_none_or(|at| at > 0),
            "Invalid scanner timestamp"
        );
        ensure!(
            self.endpoint_ping_ms.is_none_or(|ms| ms > 0 && ms <= 60_000)
                && self.tunnel_ping_ms.is_none_or(|ms| ms > 0 && ms <= 60_000)
                && self.tunnel_loss_percent.is_none_or(|loss| loss <= 100),
            "Invalid scanner measurements"
        );
        ensure!(
            self.node
                .as_ref()
                .is_none_or(|node| node.len() == 3 && node.bytes().all(|b| b.is_ascii_uppercase())),
            "Invalid WARP node"
        );
        ensure!(
            self.node_location.as_ref().is_none_or(|location| !location.is_empty()
                && location.len() <= 120
                && !location.chars().any(char::is_control)),
            "Invalid WARP node location"
        );
        ensure!(
            self.https_warp
                && (3..=20).contains(&self.samples)
                && self.failed <= self.samples.saturating_sub(3)
                && self.checked_at > 0
                && self.median_ms > 0,
            "Incomplete WARP validation"
        );
        ensure!(
            self.node_country
                .as_ref()
                .is_none_or(|c| c.len() == 2 && c.bytes().all(|b| b.is_ascii_uppercase())),
            "Invalid node country"
        );
        ensure!(
            self.exit_country
                .as_ref()
                .is_none_or(|c| c.len() == 2 && c.bytes().all(|b| b.is_ascii_uppercase())),
            "Invalid observed country"
        );
        Ok(())
    }

    fn compare(&self, other: &Self) -> Ordering {
        (u16::from(self.failed) * u16::from(other.samples))
            .cmp(&(u16::from(other.failed) * u16::from(self.samples)))
            .then(self.median_ms.cmp(&other.median_ms))
            .then(self.jitter_ms.cmp(&other.jitter_ms))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ServerSelection {
    #[default]
    Automatic,
    Manual {
        #[serde(rename = "profileId")]
        profile_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VerifiedProfile {
    pub(crate) profile: WarpProfile,
    pub(crate) observation: WarpObservation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) rejected_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScannerObservation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) network_context: Option<String>,
    pub(crate) checked_at: u64,
    pub(crate) node: Option<String>,
    pub(crate) node_country: Option<String>,
    pub(crate) node_location: Option<String>,
    pub(crate) endpoint_ping_ms: Option<u32>,
    pub(crate) tunnel_ping_ms: Option<u32>,
    pub(crate) tunnel_loss_percent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) search_measurement: Option<SearchMeasurement>,
}

impl ScannerObservation {
    pub(crate) fn validate(&self) -> Result<()> {
        if let Some(measurement) = &self.search_measurement {
            measurement.validate()?;
        }
        ensure!(
            super::warp_context::valid(self.network_context.as_deref()),
            "Invalid scanner context"
        );
        ensure!(self.checked_at > 0, "Invalid scanner timestamp");
        ensure!(
            self.node
                .as_ref()
                .is_none_or(|value| value.len() == 3 && value.bytes().all(|b| b.is_ascii_uppercase()))
                && self
                    .node_country
                    .as_ref()
                    .is_none_or(|value| value.len() == 2 && value.bytes().all(|b| b.is_ascii_uppercase()))
                && self.node_location.as_ref().is_none_or(|value| !value.is_empty()
                    && value.len() <= 120
                    && !value.chars().any(char::is_control)),
            "Invalid scanner location"
        );
        ensure!(
            self.endpoint_ping_ms.is_none_or(|ms| ms > 0 && ms <= 60_000)
                && self.tunnel_ping_ms.is_none_or(|ms| ms > 0 && ms <= 60_000)
                && self.tunnel_loss_percent.is_none_or(|loss| loss <= 100),
            "Invalid scanner measurement"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScannedProfile {
    pub(crate) profile: WarpProfile,
    pub(crate) observation: ScannerObservation,
    pub(crate) rejected_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WarpCatalog {
    version: u32,
    #[serde(default)]
    registration_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    masque_registration_fingerprint: Option<String>,
    pub(crate) selection: ServerSelection,
    pub(crate) entries: Vec<VerifiedProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) scanned: Vec<ScannedProfile>,
}

impl Default for WarpCatalog {
    fn default() -> Self {
        Self {
            version: 1,
            registration_fingerprint: None,
            masque_registration_fingerprint: None,
            selection: ServerSelection::Automatic,
            entries: Vec::new(),
            scanned: Vec::new(),
        }
    }
}

impl WarpCatalog {
    pub(crate) fn bind_registration(&mut self, account: Option<&super::warp_account::WarpAccount>) {
        let fingerprint = account.map(|account| {
            Sha256::digest(account.id().as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        });
        if fingerprint.is_none() || self.registration_fingerprint != fingerprint {
            self.invalidate_family(RegistrationFamily::WireGuard);
            self.registration_fingerprint = fingerprint;
        }
    }

    pub(crate) fn bind_masque_registration(&mut self, account: Option<&super::warp_masque_account::MasqueAccount>) {
        let fingerprint = account.map(|account| account.fingerprint());
        if fingerprint.is_none() || self.masque_registration_fingerprint != fingerprint {
            self.invalidate_family(RegistrationFamily::Masque);
            self.masque_registration_fingerprint = fingerprint;
        }
    }

    fn invalidate_family(&mut self, family: RegistrationFamily) {
        let (stale, current): (Vec<_>, Vec<_>) = std::mem::take(&mut self.entries)
            .into_iter()
            .partition(|entry| entry.profile.transport.family() == family);
        self.entries = current;
        for entry in self
            .scanned
            .iter_mut()
            .filter(|entry| entry.profile.transport.family() == family)
        {
            entry.rejected_at = None;
            entry.observation.network_context = None;
        }
        for entry in stale {
            if self
                .scanned
                .iter()
                .any(|scanned| scanned.profile.id == entry.profile.id)
            {
                continue;
            }
            self.scanned.push(ScannedProfile {
                profile: entry.profile,
                observation: ScannerObservation {
                    network_context: None,
                    checked_at: entry
                        .observation
                        .scanner_checked_at
                        .unwrap_or(entry.observation.checked_at),
                    node: entry.observation.node,
                    node_country: entry.observation.node_country,
                    node_location: entry.observation.node_location,
                    endpoint_ping_ms: entry.observation.endpoint_ping_ms,
                    tunnel_ping_ms: entry.observation.tunnel_ping_ms,
                    tunnel_loss_percent: entry.observation.tunnel_loss_percent,
                    search_measurement: entry.observation.search_measurement,
                },
                rejected_at: None,
            });
        }
        while self.scanned.len() > MAX_SCANNED_PROFILES {
            let oldest = self.scanned.iter().enumerate()
                .filter(|(_, entry)| !matches!(&self.selection, ServerSelection::Manual { profile_id } if profile_id == &entry.profile.id))
                .min_by_key(|(_, entry)| (entry.observation.search_measurement.as_ref().is_some_and(|measurement| measurement.stages.https == Some(true)), entry.observation.checked_at)).map(|(index, _)| index).expect("an unpinned candidate exists");
            self.scanned.remove(oldest);
        }
    }

    pub(crate) async fn load_current(root: &Path) -> Result<Self> {
        let mut catalog = Self::load(&root.join("warp-profiles.json")).await?;
        // Unavailable identity invalidates only its own evidence; status commands report read errors separately.
        let account = super::warp_account::WarpAccountStore::new(root.into())
            .load()
            .await
            .ok()
            .flatten();
        catalog.bind_registration(account.as_ref());
        let masque = super::warp_masque_account::MasqueAccountStore::new(root.into())
            .load()
            .await
            .ok()
            .flatten();
        catalog.bind_masque_registration(masque.as_ref());
        Ok(catalog)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1 && self.entries.len() <= MAX_PROFILES && self.scanned.len() <= MAX_SCANNED_PROFILES,
            "Unsupported or oversized WARP catalog"
        );
        ensure!(
            [&self.registration_fingerprint, &self.masque_registration_fingerprint]
                .into_iter()
                .all(|fingerprint| fingerprint
                    .as_ref()
                    .is_none_or(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))),
            "Invalid WARP registration fingerprint"
        );
        let mut ids = HashSet::new();
        for entry in &self.entries {
            entry.profile.validate()?;
            entry.observation.validate()?;
            ensure!(
                entry.rejected_at.is_none_or(|at| at > 0),
                "Invalid validation timestamp"
            );
            ensure!(ids.insert(&entry.profile.id), "Duplicate WARP profile");
        }
        let mut scanned_ids = HashSet::new();
        for entry in &self.scanned {
            entry.profile.validate()?;
            entry.observation.validate()?;
            ensure!(entry.rejected_at.is_none_or(|at| at > 0), "Invalid rejection timestamp");
            ensure!(scanned_ids.insert(&entry.profile.id), "Duplicate scanner candidate");
        }
        if let ServerSelection::Manual { profile_id } = &self.selection {
            ensure!(
                ids.contains(profile_id) || scanned_ids.contains(profile_id),
                "Selected WARP profile is missing"
            );
        }
        Ok(())
    }

    pub(crate) fn select(&mut self, selection: ServerSelection) -> Result<()> {
        if let ServerSelection::Manual { profile_id } = &selection {
            ensure!(
                self.entries.iter().any(|p| p.profile.id == *profile_id)
                    || self.scanned.iter().any(|p| p.profile.id == *profile_id),
                "Selected WARP profile is missing"
            );
        }
        self.selection = selection;
        Ok(())
    }

    pub(crate) fn profile_allowed(&self, id: &str, policy: &super::warp_selection::SelectionPolicy) -> bool {
        if let Some(entry) = self.entries.iter().find(|entry| entry.profile.id == id) {
            return policy.allows_observation(&entry.profile.transport, &entry.observation, true);
        }
        self.scanned
            .iter()
            .find(|entry| entry.profile.id == id)
            .is_some_and(|entry| {
                policy.allows_transport(&entry.profile.transport)
                    && policy.matches_metadata(
                        entry.observation.node.as_deref(),
                        entry.observation.node_country.as_deref(),
                        true,
                    )
            })
    }

    pub(crate) fn selection_allowed(&self, policy: &super::warp_selection::SelectionPolicy) -> bool {
        match &self.selection {
            ServerSelection::Automatic => true,
            ServerSelection::Manual { profile_id } => self.profile_allowed(profile_id, policy),
        }
    }

    pub(crate) fn profiles_to_validate(
        &self,
        policy: &super::warp_selection::SelectionPolicy,
        context: Option<&str>,
    ) -> Vec<WarpProfile> {
        let mut profiles: Vec<_> = self
            .candidates()
            .into_iter()
            .filter(|entry| self.profile_allowed(&entry.profile.id, policy))
            .map(|entry| entry.profile.clone())
            .collect();
        let mut scanned: Vec<_> = self
            .scanned
            .iter()
            .filter(|entry| {
                self.profile_allowed(&entry.profile.id, policy)
                    && match &self.selection {
                        ServerSelection::Automatic => true,
                        ServerSelection::Manual { profile_id } => &entry.profile.id == profile_id,
                    }
            })
            .collect();
        scanned.sort_by_key(|entry| {
            (
                entry.observation.tunnel_loss_percent.unwrap_or(100),
                entry.observation.tunnel_ping_ms.unwrap_or(u32::MAX),
            )
        });
        for entry in scanned {
            if !profiles.iter().any(|profile| profile.id == entry.profile.id) {
                profiles.push(entry.profile.clone());
            }
        }
        profiles.sort_by_key(|profile| {
            let recorded = self
                .entries
                .iter()
                .find(|entry| entry.profile.id == profile.id)
                .and_then(|entry| entry.observation.network_context.as_deref())
                .or_else(|| {
                    self.scanned
                        .iter()
                        .find(|entry| entry.profile.id == profile.id)
                        .and_then(|entry| entry.observation.network_context.as_deref())
                });
            match super::warp_context::compare(recorded, context) {
                super::warp_context::ContextMatch::Same => 0,
                super::warp_context::ContextMatch::Unknown => 1,
                super::warp_context::ContextMatch::Changed => 2,
            }
        });
        profiles
    }

    pub(crate) fn is_rejected(&self, id: &str) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.profile.id == id)
            .and_then(|entry| entry.rejected_at)
            .is_some()
            || self
                .scanned
                .iter()
                .find(|entry| entry.profile.id == id)
                .and_then(|entry| entry.rejected_at)
                .is_some()
    }

    pub(crate) fn select_with_policy(
        &mut self,
        selection: ServerSelection,
        policy: &super::warp_selection::SelectionPolicy,
    ) -> Result<()> {
        if let ServerSelection::Manual { profile_id } = &selection {
            ensure!(
                self.profile_allowed(profile_id, policy),
                "Выбранный сервер не соответствует ограничениям подключения."
            );
        }
        self.select(selection)
    }

    pub(crate) fn candidates(&self) -> Vec<&VerifiedProfile> {
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| match &self.selection {
                ServerSelection::Automatic => true,
                ServerSelection::Manual { profile_id } => entry.profile.id == *profile_id,
            })
            .collect();
        entries.sort_by(|a, b| {
            a.observation
                .compare(&b.observation)
                .then(a.profile.id.cmp(&b.profile.id))
        });
        entries
    }

    pub(crate) fn record_scanned(&mut self, profile: WarpProfile, observation: ScannerObservation) -> Result<()> {
        profile.validate()?;
        observation.validate()?;
        let rejected_at = self
            .scanned
            .iter()
            .find(|entry| entry.profile.id == profile.id)
            .and_then(|entry| entry.rejected_at);
        self.scanned.retain(|entry| entry.profile.id != profile.id);
        self.scanned.push(ScannedProfile {
            profile,
            observation,
            rejected_at,
        });
        if self.scanned.len() > MAX_SCANNED_PROFILES {
            let oldest = self.scanned.iter().enumerate()
                .filter(|(_, entry)| !matches!(&self.selection, ServerSelection::Manual { profile_id } if profile_id == &entry.profile.id))
                .min_by_key(|(_, entry)| (entry.observation.search_measurement.as_ref().is_some_and(|measurement| measurement.stages.https == Some(true)), entry.observation.checked_at)).map(|(index, _)| index).expect("an unpinned candidate exists");
            self.scanned.remove(oldest);
        }
        Ok(())
    }

    pub(crate) fn record_rejection(&mut self, id: &str, at: u64) -> Result<()> {
        ensure!(at > 0, "Invalid rejection timestamp");
        let mut found = false;
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.profile.id == id) {
            entry.rejected_at = Some(at);
            found = true;
        }
        if let Some(entry) = self.scanned.iter_mut().find(|entry| entry.profile.id == id) {
            entry.rejected_at = Some(at);
            found = true;
        }
        ensure!(found, "Rejected profile is missing");
        Ok(())
    }

    pub(crate) fn record_probe(&mut self, profile: WarpProfile, mut observation: WarpObservation) -> Result<()> {
        if let Some(previous) = self.entries.iter().find(|entry| entry.profile.id == profile.id) {
            let scanner = &previous.observation;
            if observation.node.is_none() {
                observation.node = scanner.node.clone();
                observation.node_country = scanner.node_country.clone();
            }
            if observation.node == scanner.node {
                observation.node_location = scanner.node_location.clone();
            }
            observation.endpoint_ping_ms = scanner.endpoint_ping_ms;
            observation.tunnel_ping_ms = scanner.tunnel_ping_ms;
            observation.tunnel_loss_percent = scanner.tunnel_loss_percent;
            observation.scanner_checked_at = scanner.scanner_checked_at;
            observation.scanner_context = scanner.scanner_context.clone();
            observation.search_measurement = scanner.search_measurement.clone();
        }
        if let Some(scanner) = self
            .scanned
            .iter()
            .find(|entry| entry.profile.id == profile.id)
            .map(|entry| &entry.observation)
        {
            if observation.node.is_none() {
                observation.node = scanner.node.clone();
                observation.node_country = scanner.node_country.clone();
            }
            if observation.node == scanner.node {
                observation.node_location = scanner.node_location.clone();
            }
            observation.endpoint_ping_ms = scanner.endpoint_ping_ms;
            observation.tunnel_ping_ms = scanner.tunnel_ping_ms;
            observation.tunnel_loss_percent = scanner.tunnel_loss_percent;
            observation.scanner_checked_at = Some(scanner.checked_at);
            observation.scanner_context = scanner.network_context.clone();
            observation.search_measurement = scanner.search_measurement.clone();
        }
        self.record(profile, observation)
    }

    pub(crate) fn record(&mut self, profile: WarpProfile, observation: WarpObservation) -> Result<()> {
        profile.validate()?;
        observation.validate()?;
        self.scanned.retain(|entry| entry.profile.id != profile.id);
        self.entries.retain(|entry| entry.profile.id != profile.id);
        self.entries.push(VerifiedProfile {
            profile,
            observation,
            rejected_at: None,
        });
        if self.entries.len() > MAX_PROFILES {
            let remove = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| match &self.selection {
                    ServerSelection::Manual { profile_id } => e.profile.id != *profile_id,
                    ServerSelection::Automatic => true,
                })
                .min_by_key(|(_, e)| e.observation.checked_at)
                .map(|(i, _)| i)
                .expect("an unpinned entry exists");
            self.entries.remove(remove);
        }
        Ok(())
    }

    pub(crate) async fn load(path: &Path) -> Result<Self> {
        let file = match tokio::fs::File::open(path).await {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(_) => return Err(anyhow!("Unable to read WARP profile catalog")),
        };
        let mut bytes = Vec::new();
        file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes).await?;
        ensure!(bytes.len() <= MAX_BYTES, "WARP profile catalog exceeded its limit");
        let catalog: Self = serde_json::from_slice(&bytes).map_err(|_| anyhow!("Invalid WARP profile catalog"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub(crate) async fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let json = serde_json::to_string_pretty(self)?;
        ensure!(json.len() <= MAX_BYTES, "WARP profile catalog exceeded its limit");
        crate::utils::help::save_yaml_str(path, &json).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv6_transport_pools_do_not_overlap_or_widen_http3() -> anyhow::Result<()> {
        let sni = "consumer-masque.cloudflareclient.com".to_owned();
        for (transport, valid, invalid) in [
            (
                WarpTransport::WireGuard,
                "[2606:4700:d1:abcd::1]:500",
                "[2606:4700:103::1]:500",
            ),
            (
                WarpTransport::Awg {
                    jc: 4,
                    jmin: 40,
                    jmax: 70,
                    i1: "<r 32>".into(),
                },
                "[2606:4700:d0::1]:2408",
                "[2606:4700:104::1]:2408",
            ),
            (
                WarpTransport::MasqueH3 { sni: sni.clone() },
                "[2606:4700:104::2]:8443",
                "[2606:4700:104::3]:443",
            ),
            (
                WarpTransport::MasqueH2 { sni },
                "[2606:4700:104:abcd::3]:443",
                "[2606:4700:105::1]:443",
            ),
        ] {
            WarpProfile::new(valid.parse()?, transport.clone())?.validate()?;
            assert!(
                WarpProfile::new(invalid.parse()?, transport.clone()).is_err(),
                "{invalid}"
            );
            assert!(WarpProfile::new("[2606:4700:104::1%3]:443".parse()?, transport).is_err());
        }
        Ok(())
    }

    #[test]
    fn ipv4_ids_and_manual_pin_survive_dual_stack_catalog_round_trip() -> anyhow::Result<()> {
        let endpoint: std::net::SocketAddrV4 = "188.114.96.10:500".parse()?;
        let transport = WarpTransport::WireGuard;
        let old_bytes = serde_json::to_vec(&(endpoint, &transport))?;
        let old_id: String = Sha256::digest(old_bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let ipv4 = WarpProfile::new(endpoint.into(), transport)?;
        assert_eq!(ipv4.id, old_id);
        let mut catalog = WarpCatalog::default();
        catalog.record(ipv4.clone(), observation(80, 0))?;
        catalog.select(ServerSelection::Manual { profile_id: old_id })?;
        catalog.record(
            profile("[2606:4700:d0::1]:2408", WarpTransport::WireGuard),
            observation(10, 0),
        )?;
        let restored: WarpCatalog = serde_json::from_slice(&serde_json::to_vec(&catalog)?)?;
        restored.validate()?;
        assert_eq!(restored.selection, catalog.selection);
        assert_eq!(restored.candidates().len(), 1);
        assert_eq!(restored.candidates()[0].profile, ipv4);
        Ok(())
    }

    fn profile(endpoint: &str, transport: WarpTransport) -> WarpProfile {
        WarpProfile::new(endpoint.parse().unwrap(), transport).unwrap()
    }

    #[tokio::test]
    async fn legacy_wireguard_catalog_and_manual_pin_survive_unreadable_masque_identity() -> anyhow::Result<()> {
        use crate::config::warp_account::{WarpAccount, WarpAccountStore};
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let root = std::env::temp_dir().join(format!("tropa-family-catalog-{}", nanoid::nanoid!()));
        let account = WarpAccount::from_warpscout_json(&serde_json::to_vec(&serde_json::json!({
            "id":"legacy-device","token":"synthetic-token","private_key":STANDARD.encode([7u8;32]),
            "peer_public_key":STANDARD.encode([9u8;32]),"ipv4":"172.16.0.2","ipv6":null
        }))?)?;
        let store = WarpAccountStore::new(root.clone());
        store.save(&account).await?;
        let wg_before = tokio::fs::read(root.join("secrets/warp-account.json")).await?;
        let mut catalog = WarpCatalog::default();
        catalog.bind_registration(Some(&account));
        let candidate = profile("188.114.96.10:500", WarpTransport::WireGuard);
        catalog.record(candidate.clone(), observation(20, 0))?;
        catalog.select(ServerSelection::Manual {
            profile_id: candidate.id.clone(),
        })?;
        let mut legacy = serde_json::to_value(&catalog)?;
        legacy
            .as_object_mut()
            .unwrap()
            .remove("masque_registration_fingerprint");
        tokio::fs::write(root.join("warp-profiles.json"), serde_json::to_vec(&legacy)?).await?;
        tokio::fs::write(root.join("secrets/masque-account.json"), b"invalid-synthetic-masque").await?;
        let current = WarpCatalog::load_current(&root).await?;
        assert_eq!(current.entries, catalog.entries);
        assert_eq!(current.selection, catalog.selection);
        assert_eq!(current.candidates()[0].profile, candidate);
        current.save(&root.join("warp-profiles.json")).await?;
        assert_eq!(WarpCatalog::load_current(&root).await?, current);
        assert_eq!(
            tokio::fs::read(root.join("secrets/warp-account.json")).await?,
            wg_before
        );
        assert_eq!(
            tokio::fs::read(root.join("secrets/masque-account.json")).await?,
            b"invalid-synthetic-masque"
        );
        for name in ["warp-account.json", "masque-account.json"] {
            tokio::fs::remove_file(root.join("secrets").join(name)).await?;
        }
        tokio::fs::remove_file(root.join("warp-profiles.json")).await?;
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }

    #[test]
    fn registration_changes_only_invalidate_their_family_and_preserve_manual_endpoint() -> anyhow::Result<()> {
        use crate::config::warp_masque_account::{MasqueAccount, MasqueKeys, PendingMasque};
        let masque = |id| -> anyhow::Result<MasqueAccount> {
            let keys = MasqueKeys::generate()?;
            let peer = serde_json::to_value(MasqueKeys::generate()?)?["public_key"]
                .as_str()
                .unwrap()
                .to_owned();
            let pending = PendingMasque::from_created(
                keys,
                &serde_json::to_vec(&serde_json::json!({"id":id,"token":"synthetic-token"}))?,
            )?;
            MasqueAccount::from_enrollment(
                pending,
                &serde_json::to_vec(&serde_json::json!({"config":{
                    "peers":[{"public_key":format!("-----BEGIN PUBLIC KEY-----\n{peer}\n-----END PUBLIC KEY-----")}],
                    "interface":{"addresses":{"v4":"172.16.0.2","v6":""}}
                }}))?,
            )
        };
        let first = masque("masque-device-a")?;
        let second = masque("masque-device-b")?;
        let mut catalog = WarpCatalog::default();
        catalog.bind_masque_registration(Some(&first));
        let wg = profile("188.114.96.10:500", WarpTransport::WireGuard);
        let awg = profile(
            "188.114.96.11:500",
            WarpTransport::Awg {
                jc: 6,
                jmin: 10,
                jmax: 50,
                i1: "<r 20>".into(),
            },
        );
        let h3 = profile(
            "162.159.198.1:443",
            WarpTransport::MasqueH3 {
                sni: "consumer-masque.cloudflareclient.com".into(),
            },
        );
        let h2 = profile(
            "162.159.198.2:443",
            WarpTransport::MasqueH2 {
                sni: "consumer-masque.cloudflareclient.com".into(),
            },
        );
        for profile in [&wg, &awg, &h3, &h2] {
            catalog.record(profile.clone(), observation(20, 0))?;
        }
        catalog.record_rejection(&h3.id, 200)?;
        let manual = ServerSelection::Manual {
            profile_id: h3.id.clone(),
        };
        catalog.select(manual.clone())?;
        catalog.bind_masque_registration(Some(&first));
        assert_eq!(catalog.entries.len(), 4);
        catalog.bind_masque_registration(Some(&second));
        catalog.validate()?;
        assert_eq!(
            catalog.entries.iter().map(|entry| &entry.profile).collect::<Vec<_>>(),
            vec![&wg, &awg]
        );
        assert_eq!(catalog.scanned.len(), 2);
        assert_eq!(catalog.selection, manual);
        assert!(!catalog.is_rejected(&h3.id));
        assert_eq!(
            catalog.profiles_to_validate(&Default::default(), None),
            vec![h3.clone()]
        );
        let json = serde_json::to_string(&catalog)?;
        assert!(!json.contains("masque-device"));
        assert!(!json.contains("synthetic-token"));
        catalog.record(h3.clone(), observation(25, 0))?;
        let replacement_key_same_device = masque("masque-device-b")?;
        catalog.bind_masque_registration(Some(&replacement_key_same_device));
        assert_eq!(catalog.entries.len(), 2);
        assert_eq!(catalog.scanned.len(), 2);
        catalog.record(h2.clone(), observation(35, 0))?;
        catalog.bind_registration(None);
        assert_eq!(
            catalog.entries.iter().map(|entry| &entry.profile).collect::<Vec<_>>(),
            vec![&h2]
        );
        assert_eq!(catalog.scanned.len(), 3);
        assert_eq!(catalog.selection, manual);
        Ok(())
    }

    #[tokio::test]
    async fn scanner_only_candidates_survive_restart_without_becoming_verified() -> anyhow::Result<()> {
        let candidate = profile("188.114.96.10:500", WarpTransport::WireGuard);
        let mut json = serde_json::to_value(WarpCatalog::default())?;
        json["scanned"] = serde_json::json!([{
            "profile": candidate, "observation": {
                "checkedAt": 123, "node": "HEL", "nodeCountry": "FI",
                "nodeLocation": "Helsinki, FI", "endpointPingMs": 20,
                "tunnelPingMs": 25, "tunnelLossPercent": 0
            }, "rejectedAt": null
        }]);
        let mut catalog: WarpCatalog = serde_json::from_value(json).expect("scanner-only rows must be readable");
        catalog.validate()?;
        assert!(catalog.candidates().is_empty());
        catalog.select(ServerSelection::Manual {
            profile_id: candidate.id.clone(),
        })?;
        assert_eq!(
            catalog.profiles_to_validate(&Default::default(), None),
            vec![candidate.clone()]
        );
        let directory = std::env::temp_dir().join(format!("tropa-scanned-{}", nanoid::nanoid!()));
        tokio::fs::create_dir(&directory).await?;
        let path = directory.join("warp-profiles.json");
        catalog.save(&path).await?;
        let mut restored = WarpCatalog::load(&path).await?;
        assert_eq!(restored, catalog);
        restored.record(candidate, observation(30, 0))?;
        assert_eq!(restored.candidates().len(), 1);
        assert!(serde_json::to_value(&restored)?.get("scanned").is_none());
        let selected = ServerSelection::Manual {
            profile_id: restored.entries[0].profile.id.clone(),
        };
        restored.select(selected.clone())?;
        let profile = restored.entries[0].profile.clone();
        restored.record_rejection(&profile.id, 200)?;
        restored.select_with_policy(selected.clone(), &Default::default())?;
        assert_eq!(restored.selection, selected);
        restored.save(&path).await?;
        let mut restored = WarpCatalog::load(&path).await?;
        assert!(restored.is_rejected(&profile.id));
        restored.record(profile, observation(30, 0))?;
        assert!(!restored.is_rejected(&restored.entries[0].profile.id));
        tokio::fs::remove_file(path).await?;
        tokio::fs::remove_dir(directory).await?;
        Ok(())
    }

    #[test]
    fn policy_changes_preserve_pinned_choice_but_forbid_a_restricted_selection() -> anyhow::Result<()> {
        let mut catalog = WarpCatalog::default();
        let candidate = profile("188.114.96.10:500", WarpTransport::WireGuard);
        let mut checked = observation(20, 0);
        checked.node = Some("DME".into());
        checked.node_country = Some("RU".into());
        catalog.record(candidate.clone(), checked)?;
        let pinned = ServerSelection::Manual {
            profile_id: candidate.id,
        };
        catalog.select(pinned.clone())?;
        let policy = super::super::warp_selection::SelectionPolicy {
            exclude_nodes: vec!["DME".into()],
            ..Default::default()
        };
        assert!(!catalog.selection_allowed(&policy));
        assert_eq!(catalog.selection, pinned);
        assert!(catalog.select_with_policy(pinned.clone(), &policy).is_err());
        assert_eq!(catalog.selection, pinned);
        catalog.select_with_policy(ServerSelection::Automatic, &policy)?;
        assert!(catalog.selection_allowed(&policy));
        Ok(())
    }

    #[test]
    fn old_country_field_migrates_only_to_exit_country() -> anyhow::Result<()> {
        let mut json = serde_json::to_value(observation(20, 0))?;
        let old = json.as_object_mut().unwrap().remove("exitCountry").unwrap();
        json.as_object_mut().unwrap().insert("country".into(), old);
        json.as_object_mut().unwrap().remove("nodeCountry");
        let restored: WarpObservation = serde_json::from_value(json)?;
        assert_eq!(restored.exit_country.as_deref(), Some("FI"));
        assert_eq!(restored.node_country, None);
        let policy = super::super::warp_selection::SelectionPolicy {
            include_node_countries: vec!["FI".into()],
            ..Default::default()
        };
        assert!(!policy.allows_observation(&WarpTransport::WireGuard, &restored, false));
        Ok(())
    }

    #[test]
    fn changing_registration_invalidates_observations_but_reuse_preserves_them() -> anyhow::Result<()> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let account = |id| {
            super::super::warp_account::WarpAccount::from_warpscout_json(
                &serde_json::to_vec(&serde_json::json!({
                    "id":id,"token":"synthetic-token","private_key":STANDARD.encode([7u8;32]),
                    "peer_public_key":STANDARD.encode([9u8;32]),"ipv4":"172.16.0.2","ipv6":null
                }))
                .unwrap(),
            )
        };
        let first = account("synthetic-device-a")?;
        let second = account("synthetic-device-b")?;
        let mut catalog = WarpCatalog::default();
        catalog.bind_registration(Some(&first));
        let candidate = profile("188.114.96.10:500", WarpTransport::WireGuard);
        let mut measured = observation(20, 0);
        measured.network_context = Some("a".repeat(64));
        measured.scanner_context = Some("a".repeat(64));
        catalog.record(candidate.clone(), measured)?;
        catalog.select(ServerSelection::Manual {
            profile_id: candidate.id,
        })?;
        catalog.bind_registration(Some(&first));
        assert_eq!(catalog.candidates().len(), 1);
        assert!(!serde_json::to_string(&catalog)?.contains("synthetic-device"));
        let selection = catalog.selection.clone();
        catalog.bind_registration(Some(&second));
        assert!(catalog.entries.is_empty());
        assert_eq!(catalog.scanned.len(), 1);
        assert_eq!(catalog.scanned[0].observation.network_context, None);
        assert_eq!(catalog.selection, selection);
        Ok(())
    }

    #[test]
    fn catalog_writer_is_exclusive_across_file_handles_and_released_on_drop() -> anyhow::Result<()> {
        let directory = std::env::temp_dir().join(format!("tropa-catalog-lock-{}", nanoid::nanoid!()));
        std::fs::create_dir(&directory)?;
        let first = WarpCatalogLock::acquire(&directory)?;
        assert!(WarpCatalogLock::acquire(&directory).is_err());
        drop(first);
        drop(WarpCatalogLock::acquire(&directory)?);
        std::fs::remove_file(directory.join("warp-profiles.lock"))?;
        std::fs::remove_dir(directory)?;
        Ok(())
    }

    #[test]
    fn cached_validation_preserves_scanner_measurements_and_their_original_time() -> anyhow::Result<()> {
        let profile = profile("188.114.96.10:500", WarpTransport::WireGuard);
        let mut catalog = WarpCatalog::default();
        let mut scanned = observation(30, 0);
        scanned.node = Some("HEL".into());
        scanned.node_location = Some("Helsinki, FI".into());
        scanned.endpoint_ping_ms = Some(20);
        scanned.tunnel_ping_ms = Some(25);
        scanned.tunnel_loss_percent = Some(0);
        scanned.scanner_checked_at = Some(scanned.checked_at);
        catalog.record(profile.clone(), scanned.clone())?;
        let mut fresh = observation(10, 0);
        fresh.checked_at += 60;
        catalog.record_probe(profile, fresh.clone())?;
        let saved = &catalog.entries[0].observation;
        assert_eq!(saved.checked_at, fresh.checked_at);
        assert_eq!(saved.median_ms, 10);
        assert_eq!(saved.node, scanned.node);
        assert_eq!(saved.node_location, scanned.node_location);
        assert_eq!(saved.endpoint_ping_ms, Some(20));
        assert_eq!(saved.tunnel_ping_ms, Some(25));
        assert_eq!(saved.tunnel_loss_percent, Some(0));
        assert_eq!(saved.scanner_checked_at, scanned.scanner_checked_at);
        Ok(())
    }

    #[test]
    fn new_probe_preserves_scanner_context_without_relabelling_its_network() -> anyhow::Result<()> {
        let candidate = profile("188.114.96.10:500", WarpTransport::WireGuard);
        let mut catalog = WarpCatalog::default();
        let mut old = observation(20, 0);
        old.network_context = Some("a".repeat(64));
        old.scanner_context = Some("b".repeat(64));
        old.scanner_checked_at = Some(old.checked_at);
        catalog.record(candidate.clone(), old)?;
        let mut fresh = observation(10, 0);
        fresh.network_context = Some("c".repeat(64));
        catalog.record_probe(candidate, fresh)?;
        let saved = &catalog.entries[0].observation;
        assert_eq!(saved.network_context.as_deref(), Some("c".repeat(64).as_str()));
        assert_eq!(saved.scanner_context.as_deref(), Some("b".repeat(64).as_str()));
        let encoded = serde_json::to_vec(&catalog)?;
        let decoded: WarpCatalog = serde_json::from_slice(&encoded)?;
        assert_eq!(decoded, catalog);
        Ok(())
    }

    #[test]
    fn cache_prefers_current_context_but_keeps_exact_manual_selection() -> anyhow::Result<()> {
        let old = profile("188.114.96.10:500", WarpTransport::WireGuard);
        let current = profile("188.114.96.11:500", WarpTransport::WireGuard);
        let mut catalog = WarpCatalog::default();
        let mut previous = observation(5, 0);
        previous.network_context = Some("a".repeat(64));
        catalog.record(old.clone(), previous)?;
        let mut recent = observation(40, 0);
        recent.network_context = Some("b".repeat(64));
        catalog.record(current.clone(), recent)?;
        assert_eq!(
            catalog.profiles_to_validate(&Default::default(), Some(&"b".repeat(64))),
            vec![current, old.clone()]
        );
        catalog.select(ServerSelection::Manual {
            profile_id: old.id.clone(),
        })?;
        assert_eq!(
            catalog.profiles_to_validate(&Default::default(), Some(&"b".repeat(64))),
            vec![old]
        );
        Ok(())
    }

    fn observation(latency: u32, failed: u8) -> WarpObservation {
        WarpObservation {
            network_context: None,
            scanner_context: None,
            checked_at: 1_793_066_400,
            https_warp: true,
            samples: 5,
            failed,
            median_ms: latency,
            jitter_ms: 3,
            exit_country: Some("FI".into()),
            node_country: None,
            node: None,
            node_location: None,
            endpoint_ping_ms: None,
            tunnel_ping_ms: None,
            tunnel_loss_percent: None,
            scanner_checked_at: None,
            search_measurement: None,
        }
    }

    #[test]
    fn search_evidence_survives_verification_without_inventing_udp_or_losing_precision() -> Result<()> {
        let candidate = profile("162.159.198.2:443", WarpTransport::MasqueH2 { sni: "www.google.com".into() });
        let scanned: ScannerObservation = serde_json::from_value(serde_json::json!({
            "networkContext":"a".repeat(64),"checkedAt":123,"node":"HEL","nodeCountry":"FI","nodeLocation":null,
            "endpointPingMs":2,"tunnelPingMs":4,"tunnelLossPercent":20,
            "searchMeasurement":{
                "engineBuildId":format!("source:{}", "b".repeat(64)),
                "stages":{"handshake":true,"http":true,"https":true,"udp":null},
                "endpointPingMs":1.75,"tunnelPingMs":3.625,"lossPercent":20.0,"jitterMs":0.125,
                "samples":5,"method":"icmp-v4-sequential-v1:1.1.1.1","exitCountry":"DE"
            }
        }))?;
        let mut catalog = WarpCatalog::default();
        for field in ["engineBuildId", "handshake", "lossPercent", "method", "exitCountry", "tunnelPingMs"] {
            let mut invalid = scanned.clone();
            let measurement = invalid.search_measurement.as_mut().unwrap();
            match field {
                "engineBuildId" => measurement.engine_build_id = "unbound-build".into(),
                "handshake" => measurement.stages.handshake = Some(false),
                "lossPercent" => measurement.loss_percent = Some(101.0),
                "method" => measurement.method = None,
                "exitCountry" => measurement.exit_country = Some("not-a-country".into()),
                "tunnelPingMs" => measurement.tunnel_ping_ms = Some(f64::NAN),
                _ => unreachable!(),
            }
            assert!(catalog.record_scanned(candidate.clone(), invalid).is_err(), "{field}");
            assert!(catalog.scanned.is_empty());
        }
        catalog.record_scanned(candidate.clone(), scanned)?;
        assert!(catalog.entries.is_empty() && catalog.candidates().is_empty());
        let encoded = serde_json::to_vec(&catalog)?;
        let mut restored: WarpCatalog = serde_json::from_slice(&encoded)?;
        restored.record_probe(candidate, observation(20, 0))?;
        let value = serde_json::to_value(&restored)?;
        let evidence = &value["entries"][0]["observation"]["searchMeasurement"];
        assert_eq!(evidence["endpointPingMs"], 1.75);
        assert_eq!(evidence["tunnelPingMs"], 3.625);
        assert_eq!(evidence["jitterMs"], 0.125);
        assert_eq!(evidence["method"], "icmp-v4-sequential-v1:1.1.1.1");
        assert_eq!(evidence["stages"]["https"], true);
        assert!(evidence["stages"]["udp"].is_null());
        assert_eq!(evidence["exitCountry"], "DE");
        assert_eq!(value["entries"][0]["observation"]["exitCountry"], "FI");
        assert_eq!(value["entries"][0]["observation"]["scannerCheckedAt"], 123);
        let round_trip: WarpCatalog = serde_json::from_value(value)?;
        assert_eq!(round_trip, restored);
        Ok(())
    }

    #[test]
    fn bounded_scanner_catalog_keeps_https_candidates_when_later_probes_fail() -> Result<()> {
        let good = profile("162.159.198.2:443", WarpTransport::MasqueH2 { sni: "www.google.com".into() });
        let mut scanned: ScannerObservation = serde_json::from_value(serde_json::json!({
            "networkContext":null,"checkedAt":123,"node":null,"nodeCountry":null,"nodeLocation":null,
            "endpointPingMs":null,"tunnelPingMs":null,"tunnelLossPercent":null,
            "searchMeasurement":{
                "engineBuildId":format!("source:{}", "b".repeat(64)),
                "stages":{"handshake":true,"http":true,"https":true,"udp":null},
                "endpointPingMs":null,"tunnelPingMs":null,"lossPercent":null,"jitterMs":null,
                "samples":0,"method":null,"exitCountry":null
            }
        }))?;
        let mut catalog = WarpCatalog::default();
        catalog.record_scanned(good.clone(), scanned.clone())?;
        scanned.checked_at = 124;
        scanned.search_measurement.as_mut().unwrap().stages = SearchStages { handshake: Some(false), http: None, https: None, udp: None };
        for host in 0..=255 {
            catalog.record_scanned(profile(&format!("188.114.96.{host}:500"), WarpTransport::WireGuard), scanned.clone())?;
        }
        assert_eq!(catalog.scanned.len(), 256);
        assert!(catalog.scanned.iter().any(|entry| entry.profile.id == good.id));
        assert!(catalog.entries.is_empty());
        Ok(())
    }

    #[test]
    fn manual_selection_does_not_fall_back_to_a_faster_profile() -> anyhow::Result<()> {
        let slow = profile("188.114.96.10:500", WarpTransport::WireGuard);
        let fast = profile("188.114.96.11:500", WarpTransport::WireGuard);
        let mut catalog = WarpCatalog::default();
        catalog.record(slow.clone(), observation(80, 0))?;
        catalog.record(fast.clone(), observation(10, 0))?;
        assert_eq!(catalog.candidates()[0].profile.id, fast.id);
        catalog.select(ServerSelection::Manual {
            profile_id: slow.id.clone(),
        })?;
        assert_eq!(catalog.candidates().len(), 1);
        assert_eq!(catalog.candidates()[0].profile.id, slow.id);
        assert!(
            catalog
                .select(ServerSelection::Manual {
                    profile_id: "absent".into()
                })
                .is_err()
        );
        assert_eq!(catalog.candidates()[0].profile.id, slow.id);
        Ok(())
    }

    #[test]
    fn incomplete_checks_are_rejected_and_loss_precedes_latency() -> anyhow::Result<()> {
        let stable = profile("188.114.96.10:500", WarpTransport::WireGuard);
        let lossy = profile("188.114.96.11:500", WarpTransport::WireGuard);
        let mut catalog = WarpCatalog::default();
        catalog.record(lossy, observation(5, 2))?;
        catalog.record(stable.clone(), observation(80, 0))?;
        assert_eq!(catalog.candidates()[0].profile.id, stable.id);
        let mut incomplete = observation(1, 0);
        incomplete.https_warp = false;
        assert!(catalog.record(stable.clone(), incomplete).is_err());
        let mut incomplete = observation(1, 5);
        incomplete.median_ms = 0;
        assert!(catalog.record(stable, incomplete).is_err());
        assert_eq!(catalog.entries.len(), 2);
        assert!(valid_init_chain("<r 2><b 0xabcd><rc 10><rd 4>"));
        assert!(!valid_init_chain("<r 999999>"));
        assert!(!valid_init_chain("<b 0xabc>"));
        Ok(())
    }

    #[tokio::test]
    async fn complete_awg_parameters_round_trip_and_bad_save_preserves_catalog() -> anyhow::Result<()> {
        let directory = std::env::temp_dir().join(format!("tropa-profiles-{}", nanoid::nanoid!()));
        tokio::fs::create_dir(&directory).await?;
        let path = directory.join("warp-profiles.json");
        let candidate = profile(
            "8.39.204.212:1701",
            WarpTransport::Awg {
                jc: 6,
                jmin: 10,
                jmax: 50,
                i1: "<b 0x0102abcd>".into(),
            },
        );
        let mut catalog = WarpCatalog::default();
        catalog.record(candidate.clone(), observation(25, 0))?;
        catalog.select(ServerSelection::Manual {
            profile_id: candidate.id.clone(),
        })?;
        catalog.save(&path).await?;
        let saved = tokio::fs::read(&path).await?;
        assert_eq!(WarpCatalog::load(&path).await?, catalog);
        catalog.entries[0].profile.endpoint = "127.0.0.1:80".parse()?;
        assert!(catalog.save(&path).await.is_err());
        assert_eq!(tokio::fs::read(&path).await?, saved);
        tokio::fs::remove_file(path).await?;
        tokio::fs::remove_dir(directory).await?;
        Ok(())
    }
}

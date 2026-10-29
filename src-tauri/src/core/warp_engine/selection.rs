use crate::config::{
    warp_profiles::{TransportKind, WarpCatalog, WarpProfile},
    warp_selection::SelectionPolicy,
};
use std::collections::{HashSet, VecDeque};

pub(crate) fn validation_queue(
    baselines: &[WarpProfile],
    catalog: &WarpCatalog,
    policy: &SelectionPolicy,
    context: Option<&str>,
    now: u64,
) -> Vec<WarpProfile> {
    let mut candidates = catalog.profiles_to_validate(policy, context);
    if !matches!(
        catalog.selection,
        crate::config::warp_profiles::ServerSelection::Automatic
    ) {
        return candidates;
    }
    let rank = |profile: &WarpProfile| {
        let scanned = catalog.scanned.iter().find(|entry| entry.profile.id == profile.id);
        let verified = catalog.entries.iter().find(|entry| entry.profile.id == profile.id);
        let (measurement, recorded, at) = if let Some(entry) = scanned {
            (
                entry.observation.search_measurement.as_ref(),
                entry.observation.network_context.as_deref(),
                entry.observation.checked_at,
            )
        } else if let Some(entry) = verified {
            (
                entry.observation.search_measurement.as_ref(),
                entry.observation.scanner_context.as_deref(),
                entry.observation.scanner_checked_at.unwrap_or(0),
            )
        } else {
            (None, None, 0)
        };
        let freshness = match crate::config::warp_context::compare(recorded, context) {
            crate::config::warp_context::ContextMatch::Same if at <= now && now - at <= 900 => 0,
            crate::config::warp_context::ContextMatch::Changed => 2,
            _ => 1,
        };
        let comparable = measurement
            .filter(|value| value.samples >= 3 && value.method.as_deref() == Some("icmp-v4-sequential-v1:1.1.1.1"));
        (
            freshness,
            measurement.map_or(2, |value| if value.stages.https == Some(true) { 0 } else { 1 }),
            comparable.and_then(|value| value.loss_percent).unwrap_or(f64::INFINITY),
            comparable
                .and_then(|value| value.tunnel_ping_ms)
                .unwrap_or(f64::INFINITY),
            comparable.and_then(|value| value.jitter_ms).unwrap_or(f64::INFINITY),
        )
    };
    candidates.sort_by(|left, right| {
        let a = rank(left);
        let b = rank(right);
        a.0.cmp(&b.0)
            .then(a.1.cmp(&b.1))
            .then(a.2.total_cmp(&b.2))
            .then(a.3.total_cmp(&b.3))
            .then(a.4.total_cmp(&b.4))
            .then(left.id.cmp(&right.id))
    });
    let allowed = |profile: &&WarpProfile| {
        policy.allows_transport(&profile.transport)
            && (!catalog.entries.iter().any(|entry| entry.profile.id == profile.id)
                && !catalog.scanned.iter().any(|entry| entry.profile.id == profile.id)
                || catalog.profile_allowed(&profile.id, policy))
    };
    let mut groups = Vec::new();
    for kind in [
        TransportKind::WireGuard,
        TransportKind::Awg,
        TransportKind::MasqueH3,
        TransportKind::MasqueH2,
    ] {
        if !policy.allows_kind(kind) {
            continue;
        }
        let standard: Vec<_> = baselines
            .iter()
            .filter(|profile| profile.transport.kind() == kind)
            .filter(allowed)
            .collect();
        let measured: Vec<_> = candidates
            .iter()
            .filter(|profile| profile.transport.kind() == kind)
            .collect();
        let mut ordered = VecDeque::new();
        let mut seen = HashSet::new();
        for profile in standard
            .iter()
            .take(1)
            .copied()
            .chain(measured.iter().take(1).copied())
            .chain(standard.iter().skip(1).copied())
            .chain(measured.iter().skip(1).copied())
        {
            if seen.insert(profile.id.clone()) {
                ordered.push_back(profile.clone());
            }
        }
        groups.push(ordered);
    }
    let mut result = Vec::new();
    loop {
        let before = result.len();
        for group in &mut groups {
            if let Some(profile) = group.pop_front() {
                result.push(profile);
            }
        }
        if result.len() == before || result.len() >= 128 {
            result.truncate(128);
            return result;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::warp_profiles::{ScannerObservation, ServerSelection, WarpTransport};

    fn scanned(
        catalog: &mut WarpCatalog,
        endpoint: &str,
        transport: WarpTransport,
        ping: f64,
        node: &str,
    ) -> WarpProfile {
        let profile = WarpProfile::new(endpoint.parse().unwrap(), transport).unwrap();
        let observation: ScannerObservation = serde_json::from_value(serde_json::json!({
            "networkContext":"a".repeat(64),"checkedAt":123,"node":node,"nodeCountry":"FI","nodeLocation":null,
            "endpointPingMs":null,"tunnelPingMs":null,"tunnelLossPercent":null,
            "searchMeasurement":{
                "engineBuildId":format!("source:{}", "b".repeat(64)),
                "stages":{"handshake":true,"http":true,"https":true,"udp":null},
                "endpointPingMs":null,"tunnelPingMs":ping,"lossPercent":0.0,"jitterMs":0.125,
                "samples":5,"method":"icmp-v4-sequential-v1:1.1.1.1","exitCountry":"DE"
            }
        }))
        .unwrap();
        catalog.record_scanned(profile.clone(), observation).unwrap();
        profile
    }

    #[test]
    fn validation_interleaves_standard_and_precise_measured_alternatives_without_replacing_manual_pin() {
        let awg = WarpTransport::Awg {
            jc: 4,
            jmin: 40,
            jmax: 70,
            i1: String::new(),
        };
        let h3 = WarpTransport::MasqueH3 {
            sni: "consumer-masque.cloudflareclient.com".into(),
        };
        let h2 = WarpTransport::MasqueH2 {
            sni: "consumer-masque.cloudflareclient.com".into(),
        };
        let baselines = [
            WarpProfile::new("188.114.98.58:2408".parse().unwrap(), WarpTransport::WireGuard).unwrap(),
            WarpProfile::new("188.114.98.58:2408".parse().unwrap(), awg.clone()).unwrap(),
            WarpProfile::new("162.159.198.2:443".parse().unwrap(), h3).unwrap(),
            WarpProfile::new("162.159.198.2:443".parse().unwrap(), h2.clone()).unwrap(),
            WarpProfile::new("162.159.198.1:443".parse().unwrap(), h2.clone()).unwrap(),
        ];
        let mut catalog = WarpCatalog::default();
        for host in 1..=30 {
            scanned(
                &mut catalog,
                &format!("188.114.96.{host}:500"),
                awg.clone(),
                host as f64,
                "HEL",
            );
        }
        let slower = scanned(&mut catalog, "162.159.199.38:443", h2.clone(), 3.75, "HEL");
        let faster = scanned(&mut catalog, "162.159.199.155:443", h2.clone(), 3.625, "HEL");
        let excluded = scanned(&mut catalog, "162.159.198.9:443", h2, 0.1, "DME");
        let policy = SelectionPolicy {
            exclude_nodes: vec!["DME".into()],
            ..Default::default()
        };
        let queue = validation_queue(&baselines, &catalog, &policy, Some(&"a".repeat(64)), 124);
        assert_eq!(&queue[..4], &baselines[..4]);
        assert_eq!(queue[5], faster);
        assert_eq!(queue[7], baselines[4]);
        assert!(queue.iter().position(|p| p.id == faster.id) < queue.iter().position(|p| p.id == slower.id));
        assert!(!queue.contains(&excluded));
        assert_eq!(
            queue
                .iter()
                .map(|profile| &profile.id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            queue.len()
        );
        catalog
            .select(ServerSelection::Manual {
                profile_id: slower.id.clone(),
            })
            .unwrap();
        assert_eq!(validation_queue(&baselines, &catalog, &policy, None, 124), vec![slower]);
        let only_h2 = SelectionPolicy {
            allowed_transports: vec![TransportKind::MasqueH2],
            ..Default::default()
        };
        catalog.selection = ServerSelection::Automatic;
        assert!(
            validation_queue(&baselines, &catalog, &only_h2, None, 124)
                .iter()
                .all(|p| p.transport.kind() == TransportKind::MasqueH2)
        );
    }

    #[test]
    fn selection_prefers_fresh_comparable_over_stale_or_incompatible_method() {
        let h2 = WarpTransport::MasqueH2 {
            sni: "consumer-masque.cloudflareclient.com".into(),
        };
        let mut catalog = WarpCatalog::default();
        let now = 2000;

        let fresh_profile = WarpProfile::new("162.159.199.10:443".parse().unwrap(), h2.clone()).unwrap();
        let fresh_obs: ScannerObservation = serde_json::from_value(serde_json::json!({
            "networkContext":"a".repeat(64),"checkedAt":now - 5,"node":"HEL","nodeCountry":"FI","nodeLocation":null,
            "endpointPingMs":null,"tunnelPingMs":15,"tunnelLossPercent":0,
            "searchMeasurement":{
                "engineBuildId":format!("source:{}", "b".repeat(64)),
                "stages":{"handshake":true,"http":true,"https":true,"udp":null},
                "endpointPingMs":null,"tunnelPingMs":15.0,"lossPercent":0.0,"jitterMs":0.1,
                "samples":5,"method":"icmp-v4-sequential-v1:1.1.1.1","exitCountry":"DE"
            }
        }))
        .unwrap();
        catalog.record_scanned(fresh_profile.clone(), fresh_obs).unwrap();

        let stale_profile = WarpProfile::new("162.159.199.20:443".parse().unwrap(), h2.clone()).unwrap();
        let stale_obs: ScannerObservation = serde_json::from_value(serde_json::json!({
            "networkContext":"a".repeat(64),"checkedAt":now - 1000,"node":"HEL","nodeCountry":"FI","nodeLocation":null,
            "endpointPingMs":null,"tunnelPingMs":1,"tunnelLossPercent":0,
            "searchMeasurement":{
                "engineBuildId":format!("source:{}", "b".repeat(64)),
                "stages":{"handshake":true,"http":true,"https":true,"udp":null},
                "endpointPingMs":null,"tunnelPingMs":1.0,"lossPercent":0.0,"jitterMs":0.1,
                "samples":5,"method":"icmp-v4-sequential-v1:1.1.1.1","exitCountry":"DE"
            }
        }))
        .unwrap();
        catalog.record_scanned(stale_profile.clone(), stale_obs).unwrap();

        let incomp_profile = WarpProfile::new("162.159.199.30:443".parse().unwrap(), h2.clone()).unwrap();
        let incomp_obs: ScannerObservation = serde_json::from_value(serde_json::json!({
            "networkContext":"a".repeat(64),"checkedAt":now - 5,"node":"HEL","nodeCountry":"FI","nodeLocation":null,
            "endpointPingMs":null,"tunnelPingMs":1,"tunnelLossPercent":0,
            "searchMeasurement":{
                "engineBuildId":format!("source:{}", "b".repeat(64)),
                "stages":{"handshake":true,"http":true,"https":true,"udp":null},
                "endpointPingMs":null,"tunnelPingMs":0.5,"lossPercent":0.0,"jitterMs":0.1,
                "samples":5,"method":"udp-dns-sequential-v1:1.1.1.1","exitCountry":"DE"
            }
        }))
        .unwrap();
        catalog.record_scanned(incomp_profile.clone(), incomp_obs).unwrap();

        let baselines = [WarpProfile::new("162.159.198.2:443".parse().unwrap(), h2).unwrap()];
        let queue = validation_queue(
            &baselines,
            &catalog,
            &SelectionPolicy::default(),
            Some(&"a".repeat(64)),
            now,
        );

        let pos_fresh = queue.iter().position(|p| p.id == fresh_profile.id).unwrap();
        let pos_stale = queue.iter().position(|p| p.id == stale_profile.id).unwrap();
        let pos_incomp = queue.iter().position(|p| p.id == incomp_profile.id).unwrap();

        assert!(pos_fresh < pos_stale);
        assert!(pos_fresh < pos_incomp);
    }
}

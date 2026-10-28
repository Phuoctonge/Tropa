use super::wire::{Candidate, ParameterSet};
use crate::config::warp_profiles::{ScannedProfile, ScannerObservation, SearchMeasurement, SearchStages, WarpProfile};
use anyhow::Result;

impl Candidate {
    pub(crate) fn into_scanned(
        self,
        engine_build_id: String,
        network_context: Option<String>,
        checked_at: u64,
    ) -> Result<ScannedProfile> {
        self.validate()?;
        let parameters = ParameterSet {
            name: String::new(),
            transport: self.transport,
            parameters: self.parameters,
        };
        let profile = WarpProfile::new(self.endpoint, parameters.runtime_transport()?)?;
        let legacy_ping = |value: Option<f64>| value.map(|ms| ms.round().clamp(1.0, 60_000.0) as u32);
        let observation = ScannerObservation {
            network_context,
            checked_at,
            node: self.node,
            node_country: self.node_country,
            node_location: None,
            endpoint_ping_ms: legacy_ping(self.endpoint_ping_ms),
            tunnel_ping_ms: legacy_ping(self.tunnel_ping_ms),
            tunnel_loss_percent: self.loss_percent.map(|loss| loss.round() as u8),
            search_measurement: Some(SearchMeasurement {
                engine_build_id,
                stages: SearchStages {
                    handshake: self.stages.handshake,
                    http: self.stages.http,
                    https: self.stages.https,
                    udp: self.stages.udp,
                },
                endpoint_ping_ms: self.endpoint_ping_ms,
                tunnel_ping_ms: self.tunnel_ping_ms,
                loss_percent: self.loss_percent,
                jitter_ms: self.jitter_ms,
                samples: self.samples,
                method: (!self.method.is_empty()).then_some(self.method),
                exit_country: self.exit_country,
            }),
        };
        observation.validate()?;
        Ok(ScannedProfile {
            profile,
            observation,
            rejected_at: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::config::warp_profiles::WarpCatalog;
    use anyhow::Result;

    #[test]
    fn worker_candidate_keeps_exact_parameters_and_unverified_stages() -> Result<()> {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../protocol/fixtures/search-v1.json"))?;
        let event: serde_json::Value = serde_json::from_str(fixture["cases"][1]["json"].as_str().unwrap())?;
        let build = format!("source:{}", "b".repeat(64));
        for measured in [false, true] {
            let mut payload = event["payload"].clone();
            if measured {
                payload["endpointPingMs"] = 0.25.into();
                payload["tunnelPingMs"] = 3.625.into();
                payload["lossPercent"] = 20.0.into();
                payload["jitterMs"] = 0.125.into();
                payload["samples"] = 5.into();
                payload["method"] = "icmp-v4-sequential-v1:1.1.1.1".into();
                payload["exitCountry"] = "DE".into();
            }
            let candidate: super::super::wire::Candidate = serde_json::from_value(payload)?;
            let scanned = candidate.into_scanned(build.clone(), Some("a".repeat(64)), 123)?;
            let mut catalog = WarpCatalog::default();
            catalog.record_scanned(scanned.profile, scanned.observation)?;
            assert!(catalog.entries.is_empty() && catalog.candidates().is_empty());
            let value = serde_json::to_value(&catalog)?;
            assert_eq!(
                value["scanned"][0]["profile"]["transport"]["sni"],
                "consumer-masque.cloudflareclient.com"
            );
            let observation = &value["scanned"][0]["observation"];
            let evidence = &observation["searchMeasurement"];
            assert_eq!(evidence["engineBuildId"], build);
            assert_eq!(evidence["stages"]["https"], false);
            assert!(evidence["stages"]["udp"].is_null());
            if measured {
                assert_eq!(evidence["tunnelPingMs"], 3.625);
                assert_eq!(evidence["exitCountry"], "DE");
                assert_eq!(observation["endpointPingMs"], 1);
            } else {
                assert!(evidence["tunnelPingMs"].is_null() && evidence["method"].is_null());
            }
        }
        Ok(())
    }
}

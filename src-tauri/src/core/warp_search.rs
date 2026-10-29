use super::{
    warp_failure::{FailureReason, WarpFailure},
    warp_probe::TransportUnavailable,
};
use crate::config::warp_profiles::TransportKind;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SearchState {
    Queued,
    Running,
    Completed,
    Cancelled,
    TimedOut,
    Failed,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransportSearchProgress {
    pub(crate) transport: TransportKind,
    pub(crate) state: SearchState,
    discovered: usize,
    checked: usize,
    verified: usize,
    rejected: usize,
    untested: usize,
    pub(crate) elapsed_ms: u64,
    pub(crate) failure: Option<WarpFailure>,
    pub(crate) timings: super::warp_timing::PhaseTimings,
    #[serde(skip)]
    candidates: HashMap<String, Option<bool>>,
}

impl TransportSearchProgress {
    fn new(transport: TransportKind) -> Self {
        Self {
            transport,
            state: SearchState::Queued,
            discovered: 0,
            checked: 0,
            verified: 0,
            rejected: 0,
            untested: 0,
            elapsed_ms: 0,
            failure: None,
            timings: Default::default(),
            candidates: HashMap::new(),
        }
    }

    fn recount(&mut self) {
        self.discovered = self.candidates.len();
        self.verified = self.candidates.values().filter(|value| **value == Some(true)).count();
        self.rejected = self.candidates.values().filter(|value| **value == Some(false)).count();
        self.checked = self.verified + self.rejected;
        self.untested = self.discovered - self.checked;
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchProgress {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) unavailable_tuning: Vec<super::warp_engine::wire::UnavailableTuning>,
    pub(crate) preset: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) address_family: Option<&'static str>,
    pub(crate) discovered: usize,
    pub(crate) checked: usize,
    pub(crate) verified: usize,
    pub(crate) time_limit_reached: bool,
    pub(crate) transports: Vec<TransportSearchProgress>,
}

impl Default for SearchProgress {
    fn default() -> Self {
        Self {
            unavailable_tuning: Vec::new(),
            preset: String::new(),
            address_family: None,
            discovered: 0,
            checked: 0,
            verified: 0,
            time_limit_reached: false,
            transports: vec![
                TransportSearchProgress::new(TransportKind::WireGuard),
                TransportSearchProgress::new(TransportKind::Awg),
                TransportSearchProgress::new(TransportKind::MasqueH3),
                TransportSearchProgress::new(TransportKind::MasqueH2),
            ],
        }
    }
}

impl SearchProgress {
    pub(crate) fn diagnostic_snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "discovered": self.discovered,
            "checked": self.checked,
            "verified": self.verified,
            "timeLimitReached": self.time_limit_reached,
            "transports": self.transports.iter().map(|entry| serde_json::json!({
                "transport": entry.transport,
                "state": entry.state,
                "discovered": entry.discovered,
                "checked": entry.checked,
                "verified": entry.verified,
                "rejected": entry.rejected,
                "untested": entry.untested,
                "elapsedMs": entry.elapsed_ms,
                "failure": entry.failure,
                "timings": entry.timings.snapshot(),
            })).collect::<Vec<_>>(),
        })
    }

    pub(crate) fn begin_task(&mut self, kind: TransportKind) {
        let entry = self.transport(kind);
        entry.state = SearchState::Running;
        entry.failure = None;
    }

    pub(crate) fn transport(&mut self, kind: TransportKind) -> &mut TransportSearchProgress {
        self.transports
            .iter_mut()
            .find(|entry| entry.transport == kind)
            .expect("supported search transport")
    }

    pub(crate) fn discover(&mut self, kind: TransportKind, ids: impl IntoIterator<Item = String>) {
        let entry = self.transport(kind);
        for id in ids {
            entry.candidates.entry(id).or_insert(None);
        }
        self.recount();
    }

    fn recount(&mut self) {
        for entry in &mut self.transports {
            entry.recount();
        }
        self.discovered = self.transports.iter().map(|entry| entry.discovered).sum();
        self.checked = self.transports.iter().map(|entry| entry.checked).sum();
        self.verified = self.transports.iter().map(|entry| entry.verified).sum();
    }

    pub(crate) fn record_check(&mut self, kind: TransportKind, id: &str, result: Result<(), &anyhow::Error>) {
        let value = match result {
            Ok(()) => Some(true),
            Err(error) if error.downcast_ref::<TransportUnavailable>().is_some() => Some(false),
            Err(_) => None,
        };
        let entry = self.transport(kind).candidates.entry(id.into()).or_insert(None);
        if value.is_some() {
            *entry = value;
        }
        self.recount();
    }

    pub(crate) fn fail(&mut self, kind: TransportKind, failure: WarpFailure) {
        let state = match failure.reason {
            FailureReason::Cancelled => SearchState::Cancelled,
            FailureReason::Timeout => SearchState::TimedOut,
            FailureReason::NoCandidates => SearchState::Completed,
            _ => SearchState::Failed,
        };
        let entry = self.transport(kind);
        entry.state = state;
        entry.failure = Some(failure);
    }
}

#[derive(Default)]
pub(crate) struct RegistrationFailures {
    families: [Option<WarpFailure>; 2],
}

impl RegistrationFailures {
    fn slot(kind: TransportKind) -> usize {
        match kind {
            TransportKind::WireGuard | TransportKind::Awg => 0,
            TransportKind::MasqueH3 | TransportKind::MasqueH2 => 1,
        }
    }

    pub(crate) fn remember(&mut self, kind: TransportKind, failure: WarpFailure) {
        if failure.stage == super::warp_failure::FailureStage::Registration
            && !matches!(failure.reason, FailureReason::Cancelled | FailureReason::CleanupPending)
        {
            self.families[Self::slot(kind)].get_or_insert(failure);
        }
    }

    pub(crate) fn failure(&self, kind: TransportKind) -> Option<WarpFailure> {
        self.families[Self::slot(kind)].map(|failure| failure.for_transport(kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_keep_counts_without_preset_or_candidate_strings() {
        let mut progress = SearchProgress {
            preset: "private-preset-marker".into(),
            address_family: Some("private-family-marker"),
            ..Default::default()
        };
        progress.discover(TransportKind::Awg, ["private-candidate-marker".into()]);
        progress.record_check(TransportKind::Awg, "private-candidate-marker", Ok(()));
        let report = progress.diagnostic_snapshot();
        assert!(!serde_json::to_string(&report).unwrap().contains("private-"));
        assert_eq!(report["verified"], 1);
        assert_eq!(report["transports"][1]["verified"], 1);
        assert_eq!(report["transports"][1]["transport"], "awg");
        assert!(report["transports"][1]["timings"]["cleanupMs"].is_number());
    }

    #[test]
    fn new_transport_pass_clears_previous_failure_without_losing_results() {
        let mut progress = SearchProgress::default();
        progress.discover(TransportKind::Awg, ["working".into(), "pending".into()]);
        progress.record_check(TransportKind::Awg, "working", Ok(()));
        progress.transport(TransportKind::Awg).elapsed_ms = 120;
        let failure = WarpFailure::new(
            super::super::warp_failure::FailureStage::Scanner,
            FailureReason::Timeout,
        );
        progress.fail(TransportKind::Awg, failure);
        progress.fail(TransportKind::WireGuard, failure);
        progress.begin_task(TransportKind::Awg);

        let wire = serde_json::to_value(progress).unwrap();
        let awg = &wire["transports"][1];
        assert_eq!(awg["state"], "running");
        assert!(awg["failure"].is_null());
        assert_eq!(awg["verified"], 1);
        assert_eq!(awg["untested"], 1);
        assert_eq!(awg["elapsedMs"], 120);
        assert_eq!(wire["transports"][0]["state"], "timedOut");
        assert_eq!(wire["transports"][0]["failure"]["reason"], "timeout");
    }

    #[test]
    fn registration_failures_tracks_families_independently() {
        use super::super::warp_failure::FailureStage;
        let mut failures = RegistrationFailures::default();
        let failed = WarpFailure::new(FailureStage::Registration, FailureReason::Unreachable);
        failures.remember(TransportKind::Awg, failed);

        let blocked_wg = failures.failure(TransportKind::WireGuard).unwrap();
        assert_eq!(blocked_wg.stage, FailureStage::Registration);
        assert_eq!(blocked_wg.reason, FailureReason::Unreachable);
        assert_eq!(blocked_wg.transport, Some(TransportKind::WireGuard));

        let blocked_awg = failures.failure(TransportKind::Awg).unwrap();
        assert_eq!(blocked_awg.transport, Some(TransportKind::Awg));

        assert!(failures.failure(TransportKind::MasqueH3).is_none());
        assert!(failures.failure(TransportKind::MasqueH2).is_none());

        failures.remember(
            TransportKind::MasqueH3,
            WarpFailure::new(FailureStage::Scanner, FailureReason::Timeout),
        );
        assert!(failures.failure(TransportKind::MasqueH3).is_none());

        failures.remember(
            TransportKind::MasqueH3,
            WarpFailure::new(FailureStage::Registration, FailureReason::InvalidOutput),
        );
        assert!(failures.failure(TransportKind::MasqueH3).is_some());
        assert!(failures.failure(TransportKind::MasqueH2).is_some());
    }

    #[test]
    fn masque_progress_has_independent_candidates_and_failure_state() {
        let mut progress = SearchProgress::default();
        progress.discover(TransportKind::MasqueH3, ["h3".into()]);
        progress.discover(TransportKind::MasqueH2, ["h2".into()]);
        progress.record_check(TransportKind::MasqueH3, "h3", Ok(()));
        progress.fail(
            TransportKind::MasqueH2,
            WarpFailure::new(
                super::super::warp_failure::FailureStage::Registration,
                FailureReason::Unreachable,
            ),
        );
        let wire = serde_json::to_value(progress).unwrap();
        assert_eq!(wire["discovered"], 2);
        assert_eq!(wire["verified"], 1);
        assert_eq!(wire["transports"][2]["transport"], "masqueH3");
        assert_eq!(wire["transports"][2]["verified"], 1);
        assert_eq!(wire["transports"][3]["state"], "failed");
        assert_eq!(wire["transports"][3]["untested"], 1);
    }
}

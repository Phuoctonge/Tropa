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

use std::time::Duration;

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

    pub(crate) fn pending_tasks(
        &self,
        tasks: &[(&'static str, Duration, u8)],
    ) -> anyhow::Result<Vec<(&'static str, Duration, u8)>> {
        let mut pending = Vec::new();
        for &task in tasks {
            let kind = super::warp_scout::ScoutPreset::parse(task.0)?.kind();
            if self.failure(kind).is_none() {
                pending.push(task);
            }
        }
        Ok(pending)
    }
}

pub(crate) struct FamilyScanTask {
    pub(crate) settings: (&'static str, Duration, u8),
    pub(crate) ipv6: bool,
}

pub(crate) fn scan_tasks_for_network(extended: bool, ipv6: bool, advanced: bool) -> Vec<FamilyScanTask> {
    let settings = if advanced {
        let initial = Duration::from_secs(30);
        let expansion = Duration::from_secs(40);
        vec![
            ("standard", initial, 1),
            ("wg", initial, 1),
            ("masque", initial, 1),
            ("masque-h2", initial, 1),
            ("random", expansion, 5),
            ("no-init", expansion, 5),
            ("masque-apple", expansion, 5),
            ("masque-h2-apple", expansion, 5),
            ("masque-google", expansion, 5),
            ("masque-h2-google", expansion, 5),
            ("masque-microsoft", expansion, 5),
            ("masque-h2-microsoft", expansion, 5),
            ("masque-jsdelivr", expansion, 5),
            ("masque-h2-jsdelivr", expansion, 5),
        ]
    } else {
        scan_tasks(extended)
    };
    let mut tasks = Vec::new();
    for initial in [true, false] {
        for family in [false, true].into_iter().filter(|family| !*family || ipv6) {
            tasks.extend(
                settings
                    .iter()
                    .filter(|task| (task.2 == 1) == initial)
                    .map(|&settings| FamilyScanTask { settings, ipv6: family }),
            );
        }
    }
    tasks
}

pub(crate) fn scan_tasks(extended: bool) -> Vec<(&'static str, Duration, u8)> {
    let first = Duration::from_secs(if extended { 90 } else { 30 });
    let expansion = Duration::from_secs(if extended { 40 } else { 10 });
    vec![
        ("standard", first, 1),
        ("wg", first, 1),
        ("masque", first, 1),
        ("masque-h2", first, 1),
        ("standard", expansion, 5),
        ("wg", expansion, 5),
        ("quic", expansion, 5),
        ("stun", expansion, 5),
        ("sip", expansion, 5),
        ("dns", expansion, 5),
    ]
}

pub(crate) fn task_budget(tasks: &[(&str, Duration, u8)], index: usize, remaining: Duration) -> Duration {
    let (_, initial_budget, sample) = tasks[index];
    let reserved = tasks[index + 1..]
        .iter()
        .filter(|(_, _, sample)| *sample == 1)
        .map(|(_, budget, _)| *budget)
        .sum::<Duration>();
    if sample == 1 && remaining < initial_budget + reserved {
        let initial_tasks = tasks[index..].iter().filter(|(_, _, sample)| *sample == 1).count() as u32;
        return initial_budget.min(remaining / initial_tasks);
    }
    let available = remaining.saturating_sub(reserved);
    if sample == 1 {
        initial_budget.min(available)
    } else {
        let expansions = tasks[index..].iter().filter(|(_, _, sample)| *sample != 1).count() as u32;
        available / expansions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advanced_search_keeps_time_for_tuning_and_normal_search_never_tunes() -> anyhow::Result<()> {
        let normal = scan_tasks_for_network(true, true, false);
        assert!(
            !normal
                .iter()
                .any(|task| task.settings.0 == "random" || task.settings.0 == "masque-apple")
        );
        let advanced = scan_tasks_for_network(true, true, true);
        let settings: Vec<_> = advanced.iter().map(|task| task.settings).collect();
        let mut remaining = Duration::from_secs(600);
        let mut initial = 0;
        for (index, task) in advanced.iter().enumerate() {
            let budget = task_budget(&settings, index, remaining);
            assert!(budget >= Duration::from_secs(15));
            if task.settings.2 == 1 {
                initial += 1;
                assert!(budget <= Duration::from_secs(30));
            }
            remaining -= budget;
        }
        assert_eq!(initial, 8);
        assert_eq!(remaining, Duration::ZERO);
        for family in [false, true] {
            for preset in [
                "random",
                "no-init",
                "masque-apple",
                "masque-h2-apple",
                "masque-jsdelivr",
                "masque-h2-jsdelivr",
            ] {
                assert!(
                    advanced
                        .iter()
                        .any(|task| task.ipv6 == family && task.settings.0 == preset)
                );
            }
        }
        Ok(())
    }

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
    fn dual_stack_search_reserves_each_family_before_expansions_and_keeps_ipv4_without_ipv6() -> anyhow::Result<()> {
        for ipv6 in [false, true] {
            let tasks = scan_tasks_for_network(true, ipv6, false);
            let mut remaining = Duration::from_secs(180);
            let mut started = Vec::new();
            for (index, task) in tasks.iter().enumerate() {
                let settings: Vec<_> = tasks.iter().map(|task| task.settings).collect();
                let budget = task_budget(&settings, index, remaining);
                if task.settings.2 != 1 {
                    break;
                }
                assert!(budget >= Duration::from_secs(20));
                started.push((
                    super::super::warp_scout::ScoutPreset::parse(task.settings.0)?.kind(),
                    task.ipv6,
                ));
                remaining -= budget;
            }
            for kind in [
                TransportKind::WireGuard,
                TransportKind::Awg,
                TransportKind::MasqueH3,
                TransportKind::MasqueH2,
            ] {
                assert!(started.contains(&(kind, false)));
                assert_eq!(started.contains(&(kind, true)), ipv6);
            }
            assert!(!tasks.iter().any(|task| task.ipv6 && !ipv6));
        }
        Ok(())
    }

    #[test]
    fn failed_registration_family_is_not_prepared_again_or_reserved_scan_time() -> anyhow::Result<()> {
        use super::super::warp_failure::FailureStage;
        let mut failures = RegistrationFailures::default();
        let failed = WarpFailure::new(FailureStage::Registration, FailureReason::Unreachable);
        failures.remember(TransportKind::Awg, failed);
        let pending = failures.pending_tasks(&scan_tasks(true))?;
        assert_eq!(
            pending.iter().map(|task| task.0).collect::<Vec<_>>(),
            ["masque", "masque-h2"]
        );
        assert_eq!(
            task_budget(&pending, 0, Duration::from_secs(120)),
            Duration::from_secs(60)
        );
        assert_eq!(
            task_budget(&pending, 1, Duration::from_secs(60)),
            Duration::from_secs(60)
        );
        let blocked_wg = failures.failure(TransportKind::WireGuard).unwrap();
        assert_eq!(blocked_wg.stage, FailureStage::Registration);
        assert_eq!(blocked_wg.reason, FailureReason::Unreachable);
        assert_eq!(blocked_wg.transport, Some(TransportKind::WireGuard));
        assert!(failures.failure(TransportKind::MasqueH3).is_none());
        assert_eq!(
            RegistrationFailures::default().pending_tasks(&scan_tasks(true))?.len(),
            10
        );
        Ok(())
    }

    #[test]
    fn masque_preparation_failure_skips_both_modes_but_scanner_timeouts_do_not_block_a_family() -> anyhow::Result<()> {
        use super::super::warp_failure::FailureStage;
        let mut failures = RegistrationFailures::default();
        for failure in [
            WarpFailure::new(FailureStage::Scanner, FailureReason::Timeout),
            WarpFailure::new(FailureStage::Registration, FailureReason::Cancelled),
            WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending),
        ] {
            failures.remember(TransportKind::MasqueH3, failure);
            assert_eq!(failures.pending_tasks(&scan_tasks(false))?.len(), 10);
        }
        failures.remember(
            TransportKind::MasqueH3,
            WarpFailure::new(FailureStage::Registration, FailureReason::InvalidOutput),
        );
        let pending = failures.pending_tasks(&scan_tasks(false))?;
        assert_eq!(
            pending.iter().map(|task| task.0).collect::<Vec<_>>(),
            ["standard", "wg", "standard", "wg", "quic", "stun", "sip", "dns"]
        );
        assert!(failures.failure(TransportKind::MasqueH2).is_some());
        assert!(failures.failure(TransportKind::Awg).is_none());
        Ok(())
    }

    #[test]
    fn four_modes_get_initial_time_before_awg_expansions_even_after_cache_work() -> anyhow::Result<()> {
        for (extended, seconds) in [(false, 180), (true, 600), (true, 180)] {
            let tasks = scan_tasks(extended);
            let mut remaining = Duration::from_secs(seconds);
            let mut modes = Vec::new();
            for (index, (preset, _, sample)) in tasks.iter().enumerate() {
                let budget = task_budget(&tasks, index, remaining);
                if *sample != 1 {
                    break;
                }
                assert!(budget >= Duration::from_secs(20), "starved {preset}: {budget:?}");
                modes.push(super::super::warp_scout::ScoutPreset::parse(preset)?.kind());
                remaining -= budget;
            }
            for kind in [
                TransportKind::WireGuard,
                TransportKind::Awg,
                TransportKind::MasqueH3,
                TransportKind::MasqueH2,
            ] {
                assert!(modes.contains(&kind), "missing {kind:?}");
            }
        }
        Ok(())
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

    #[test]
    fn early_completion_redistributes_unused_time_to_remaining_tasks() {
        let tasks = scan_tasks(true);
        let mut remaining = Duration::from_secs(590);
        for index in 2..tasks.len() {
            let budget = task_budget(&tasks, index, remaining);
            assert!(budget > Duration::ZERO);
            remaining -= budget;
        }
        assert_eq!(remaining, Duration::ZERO, "completed tasks stranded usable scan time");
    }

    #[test]
    fn first_task_cannot_borrow_the_unstarted_transports_reservation() {
        let tasks = [
            ("standard", Duration::from_secs(120), 1),
            ("wg", Duration::from_secs(120), 1),
        ];
        let mut remaining = Duration::from_secs(270);
        remaining -= task_budget(&tasks, 0, remaining);
        assert_eq!(task_budget(&tasks, 1, remaining), Duration::from_secs(120));
    }

    #[test]
    fn slow_first_transport_leaves_time_for_the_other_transport() {
        let mut elapsed = Duration::ZERO;
        let mut launched = Vec::new();
        for (preset, budget, _) in scan_tasks(true) {
            if elapsed >= Duration::from_secs(600) {
                break;
            }
            launched.push(preset);
            // Each scanner consumes its whole allocated slice.
            elapsed += budget;
        }
        assert!(launched.contains(&"wg"), "WG was starved: {launched:?}");
        assert!(launched.contains(&"standard"), "AWG was starved: {launched:?}");
        assert!(elapsed <= Duration::from_secs(600));
    }
}

use parking_lot::Mutex;
use std::sync::Arc;
use tokio::time::Instant;

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Preparation,
    Scan,
    Validation,
    Cleanup,
}

#[derive(Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhaseDurations {
    pub(crate) preparation_ms: u64,
    pub(crate) scan_ms: u64,
    pub(crate) validation_ms: u64,
    pub(crate) cleanup_ms: u64,
}

impl PhaseDurations {
    fn record(&mut self, phase: Phase, elapsed: std::time::Duration) {
        let target = match phase {
            Phase::Preparation => &mut self.preparation_ms,
            Phase::Scan => &mut self.scan_ms,
            Phase::Validation => &mut self.validation_ms,
            Phase::Cleanup => &mut self.cleanup_ms,
        };
        *target = target.saturating_add(elapsed.as_millis() as u64);
    }
}

#[derive(Default)]
struct TimingState {
    totals: PhaseDurations,
    active: Option<(Phase, Instant)>,
}

#[derive(Clone, Default)]
pub(crate) struct PhaseTimings(Arc<Mutex<TimingState>>);

pub(crate) struct PhaseClock(PhaseTimings);

impl PhaseTimings {
    pub(crate) fn clock(&self) -> PhaseClock {
        self.0.lock().active = Some((Phase::Preparation, Instant::now()));
        PhaseClock(self.clone())
    }

    pub(crate) fn snapshot(&self) -> PhaseDurations {
        let state = self.0.lock();
        let mut snapshot = state.totals.clone();
        if let Some((phase, started)) = state.active {
            snapshot.record(phase, started.elapsed());
        }
        snapshot
    }
}

impl serde::Serialize for PhaseTimings {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.snapshot(), serializer)
    }
}

impl PhaseClock {
    pub(crate) fn switch(&self, phase: Phase) {
        let mut state = self.0.0.lock();
        let now = Instant::now();
        if let Some((previous, started)) = state.active.take() {
            state.totals.record(previous, now - started);
        }
        state.active = Some((phase, now));
    }
}

impl Drop for PhaseClock {
    fn drop(&mut self) {
        let mut state = self.0.0.lock();
        if let Some((phase, started)) = state.active.take() {
            state.totals.record(phase, started.elapsed());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn cleanup_is_not_counted_as_scan_time_and_error_exit_keeps_live_intervals() {
        let timings = PhaseTimings::default();
        let result: Result<(), ()> = async {
            let clock = timings.clock();
            tokio::time::advance(Duration::from_secs(2)).await;
            clock.switch(Phase::Scan);
            tokio::time::advance(Duration::from_secs(5)).await;
            clock.switch(Phase::Validation);
            tokio::time::advance(Duration::from_secs(4)).await;
            clock.switch(Phase::Cleanup);
            tokio::time::advance(Duration::from_secs(3)).await;
            let live = timings.snapshot();
            assert_eq!(
                (live.preparation_ms, live.scan_ms, live.validation_ms, live.cleanup_ms),
                (2000, 5000, 4000, 3000)
            );
            Err(())
        }
        .await;
        assert!(result.is_err());
        let saved = timings.snapshot();
        tokio::time::advance(Duration::from_secs(10)).await;
        assert_eq!(timings.snapshot().cleanup_ms, 3000);
        assert_eq!(
            saved.preparation_ms + saved.scan_ms + saved.validation_ms + saved.cleanup_ms,
            14000
        );
        {
            let next = timings.clock();
            tokio::time::advance(Duration::from_secs(2)).await;
            next.switch(Phase::Validation);
            tokio::time::advance(Duration::from_secs(1)).await;
        }
        let wire = serde_json::to_value(timings).unwrap();
        assert_eq!(wire["preparationMs"], 4000);
        assert_eq!(wire["validationMs"], 5000);
        assert_eq!(wire["cleanupMs"], 3000);
    }
}

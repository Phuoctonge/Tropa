use super::{
    process::{EngineClient, WorkerArtifact},
    scheduler::SearchSchedule,
    wire::{EventPayload, Outcome, UnavailableTuning},
};
use crate::{
    config::{
        warp_profiles::{ScannedProfile, TransportKind, WarpProfile},
        warp_selection::SelectionPolicy,
    },
    core::{
        warp_failure::{FailureReason, FailureStage, WarpFailure},
        warp_probe::ProbeCancellation,
        warp_runtime::{RuntimeAccount, profile_worker_request},
    },
};
use anyhow::{Result, anyhow, ensure};
use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{sync::mpsc, time::Instant};

pub(crate) enum SearchEvent {
    Prepared(Vec<WarpProfile>),
    Unavailable(UnavailableTuning),
    Started {
        kind: TransportKind,
        parameter_name: String,
        ipv6: bool,
    },
    Candidate(ScannedProfile),
    Finished {
        kind: TransportKind,
        elapsed_ms: u64,
        timed_out: bool,
    },
    Failure {
        kind: TransportKind,
        failure: WarpFailure,
    },
}

struct Active {
    token: u64,
    kind: TransportKind,
    started: Instant,
    deadline: Instant,
    cancellation_deadline: Option<Instant>,
}

fn failure(reason: FailureReason) -> anyhow::Error {
    WarpFailure::new(FailureStage::Scanner, reason).into()
}

fn can_finish_search(policy: &SelectionPolicy, candidate: &ScannedProfile) -> bool {
    candidate
        .observation
        .search_measurement
        .as_ref()
        .is_some_and(|measurement| measurement.stages.https == Some(true))
        && policy.allows_transport(&candidate.profile.transport)
        && policy.matches_metadata(
            candidate.observation.node.as_deref(),
            candidate.observation.node_country.as_deref(),
            false,
        )
}

async fn emit(
    output: &mpsc::Sender<SearchEvent>,
    event: SearchEvent,
    cancellation: &ProbeCancellation,
    deadline: Instant,
) -> Result<()> {
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(failure(FailureReason::Cancelled)),
        result = tokio::time::timeout_at(deadline, output.send(event)) => {
            result.map_err(|_| failure(FailureReason::Timeout))?.map_err(|_| failure(FailureReason::Cancelled))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_handoff_requires_an_allowed_https_candidate() -> Result<()> {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../protocol/fixtures/search-v1.json"))?;
        let event: serde_json::Value = serde_json::from_str(fixture["cases"][1]["json"].as_str().unwrap())?;
        let mut payload = event["payload"].clone();
        payload["stages"]["https"] = true.into();
        payload["node"] = "HEL".into();
        payload["nodeCountry"] = "FI".into();
        payload["exitCountry"] = "DE".into();
        let candidate: super::super::wire::Candidate = serde_json::from_value(payload)?;
        let mut candidate = candidate.into_scanned(format!("source:{}", "a".repeat(64)), None, 123)?;
        let mut policy = SelectionPolicy {
            exclude_nodes: vec!["HEL".into()],
            ..Default::default()
        };
        assert!(!can_finish_search(&policy, &candidate));
        policy.exclude_nodes.clear();
        policy.include_node_countries = vec!["DE".into()];
        assert!(!can_finish_search(&policy, &candidate));
        policy.include_node_countries = vec!["FI".into()];
        assert!(can_finish_search(&policy, &candidate));
        candidate.observation.node_country = None;
        assert!(!can_finish_search(&policy, &candidate));
        policy.include_node_countries.clear();
        assert!(can_finish_search(&policy, &candidate));
        policy.allowed_transports = vec![TransportKind::WireGuard];
        assert!(!can_finish_search(&policy, &candidate));
        policy.allowed_transports.clear();
        candidate.observation.search_measurement.as_mut().unwrap().stages.https = Some(false);
        assert!(!can_finish_search(&policy, &candidate));
        Ok(())
    }
}

pub(crate) async fn run_search(
    root: &Path,
    runtime_root: &Path,
    policy: &SelectionPolicy,
    ipv6: bool,
    extended: bool,
    advanced: bool,
    saved: &[WarpProfile],
    start: Instant,
    seed: u64,
    cancellation: &ProbeCancellation,
    output: &mpsc::Sender<SearchEvent>,
    resume: Option<SearchSchedule>,
) -> Result<Option<SearchSchedule>> {
    let lifetime = ProbeCancellation::linked(
        cancellation,
        start + Duration::from_secs(if extended { 600 } else { 180 }),
    );
    let mut client = EngineClient::start(root, WorkerArtifact::packaged(runtime_root)?, lifetime).await?;
    let result = run_with_client(
        root,
        &mut client,
        policy,
        ipv6,
        extended,
        advanced,
        saved,
        start,
        seed,
        cancellation,
        output,
        resume,
    )
    .await;
    match client.shutdown().await {
        Err(error)
            if result.is_ok()
                || error
                    .downcast_ref::<WarpFailure>()
                    .is_some_and(|failure| failure.reason == FailureReason::CleanupPending) =>
        {
            Err(error)
        }
        _ => result,
    }
}

pub(super) async fn run_with_client(
    root: &Path,
    client: &mut EngineClient,
    policy: &SelectionPolicy,
    ipv6: bool,
    extended: bool,
    advanced: bool,
    saved: &[WarpProfile],
    start: Instant,
    seed: u64,
    cancellation: &ProbeCancellation,
    output: &mpsc::Sender<SearchEvent>,
    resume: Option<SearchSchedule>,
) -> Result<Option<SearchSchedule>> {
    let initial_search = resume.is_none();
    let early_handoff = !extended && initial_search;
    let mut plan = if let Some(plan) = resume {
        plan
    } else {
        let mut plan =
            SearchSchedule::new_with_tuning(policy, ipv6, extended, advanced, start, seed, client.parameter_sets())?;
        plan.prepend_saved(saved)?;
        plan
    };
    let delivery_deadline = plan.discovery_deadline() + Duration::from_secs(2);
    let unavailable: Vec<_> = client.unavailable_tuning().to_vec();
    if initial_search && policy.allows_kind(TransportKind::Awg) {
        for value in unavailable {
            emit(output, SearchEvent::Unavailable(value), cancellation, delivery_deadline).await?;
        }
    }
    let network_before = crate::config::warp_context::capture();
    let mut accounts: [Option<RuntimeAccount>; 2] = [None, None];
    let mut registrations: [Option<WarpFailure>; 2] = [None, None];
    for kind in [
        TransportKind::WireGuard,
        TransportKind::Awg,
        TransportKind::MasqueH3,
        TransportKind::MasqueH2,
    ] {
        if !policy.allows_kind(kind) {
            continue;
        }
        let slot = match kind {
            TransportKind::WireGuard | TransportKind::Awg => 0,
            _ => 1,
        };
        if accounts[slot].is_some() {
            continue;
        }
        if registrations[slot].is_none() {
            let transport = client
                .parameter_sets()
                .iter()
                .map(|profile| profile.runtime_transport())
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .find(|transport| transport.kind() == kind)
                .ok_or_else(|| failure(FailureReason::Unsupported))?;
            let profile = WarpProfile::new(
                crate::core::warp_targets::targets(kind, false, extended, seed)[0],
                transport,
            )?;
            let registration_deadline = plan.discovery_deadline().min(Instant::now() + Duration::from_secs(20));
            let registration = tokio::select! {
                biased;
                _ = cancellation.cancelled() => return Err(failure(FailureReason::Cancelled)),
                result = tokio::time::timeout_at(registration_deadline, RuntimeAccount::ensure(root, &profile)) => {
                    result.map_err(|_| WarpFailure::new(FailureStage::Registration, FailureReason::Timeout).into()).and_then(|result| result)
                }
            };
            match registration {
                Ok((account, _)) => accounts[slot] = Some(account),
                Err(error) => {
                    registrations[slot] = Some(WarpFailure::from_error(
                        &error,
                        FailureStage::Registration,
                        FailureReason::InvalidOutput,
                    ))
                }
            }
        }
        if let Some(failure) = registrations[slot] {
            plan.disable(kind);
            emit(
                output,
                SearchEvent::Failure {
                    kind,
                    failure: failure.for_transport(kind),
                },
                cancellation,
                delivery_deadline,
            )
            .await?;
        }
    }
    if accounts.iter().all(Option::is_none) {
        return Err(registrations
            .into_iter()
            .flatten()
            .next()
            .map(anyhow::Error::from)
            .unwrap_or_else(|| failure(FailureReason::Unsupported)));
    }
    emit(
        output,
        SearchEvent::Prepared(plan.baseline_profiles()?),
        cancellation,
        delivery_deadline,
    )
    .await?;
    let mut has_https_candidate = false;
    let mut active: HashMap<String, Active> = HashMap::new();
    loop {
        ensure!(!cancellation.is_cancelled(), failure(FailureReason::Cancelled));
        if early_handoff && plan.initial_completed() && active.is_empty() && has_https_candidate {
            return Ok(plan.can_resume(Instant::now()).then_some(plan));
        }
        while let Some(batch) = plan.admit(Instant::now()) {
            let slot = match batch.kind {
                TransportKind::WireGuard | TransportKind::Awg => 0,
                _ => 1,
            };
            let profile = WarpProfile::new(batch.targets[0], batch.transport)?;
            let account = accounts[slot]
                .as_ref()
                .ok_or_else(|| anyhow!("Missing prepared search identity"))?;
            let timeout_ms = batch.timeout_ms.min(
                batch
                    .deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .clamp(1, 30000) as u32,
            );
            let request = profile_worker_request(account.identity(), &profile, batch.targets, timeout_ms)?;
            emit(
                output,
                SearchEvent::Started {
                    kind: batch.kind,
                    parameter_name: batch.parameter_name,
                    ipv6: batch.ipv6,
                },
                cancellation,
                delivery_deadline,
            )
            .await?;
            let id = client.submit(&request).await?;
            active.insert(
                id,
                Active {
                    token: batch.token,
                    kind: batch.kind,
                    started: Instant::now(),
                    deadline: batch.deadline,
                    cancellation_deadline: None,
                },
            );
        }
        if active.is_empty() {
            return Ok(None);
        }
        let now = Instant::now();
        for (id, job) in &mut active {
            if job.cancellation_deadline.is_some_and(|deadline| now >= deadline) {
                return Err(failure(FailureReason::Timeout));
            }
            if job.cancellation_deadline.is_none() && now >= job.deadline {
                client.cancel(id).await?;
                job.cancellation_deadline = Some(Instant::now() + Duration::from_secs(2));
            }
        }
        let wake = active
            .values()
            .map(|job| job.cancellation_deadline.unwrap_or(job.deadline))
            .min()
            .expect("active jobs");
        let event = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(failure(FailureReason::Cancelled)),
            _ = tokio::time::sleep_until(wake) => continue,
            result = client.next_event() => result?,
        };
        let job = active
            .get(&event.job_id)
            .ok_or_else(|| failure(FailureReason::InvalidOutput))?;
        match event.payload {
            EventPayload::Candidate(result) => {
                let context = network_before
                    .clone()
                    .filter(|before| crate::config::warp_context::capture().as_ref() == Some(before));
                let candidate = result.into_scanned(
                    client.build_id().to_owned(),
                    context,
                    SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                )?;
                has_https_candidate |= can_finish_search(policy, &candidate);
                emit(
                    output,
                    SearchEvent::Candidate(candidate),
                    cancellation,
                    delivery_deadline,
                )
                .await?;
            }
            EventPayload::JobFinished(outcome) => {
                let timed_out = job.cancellation_deadline.is_some();
                ensure!(
                    timed_out || outcome == Outcome::Completed,
                    failure(FailureReason::InvalidOutput)
                );
                emit(
                    output,
                    SearchEvent::Finished {
                        kind: job.kind,
                        elapsed_ms: job.started.elapsed().as_millis() as u64,
                        timed_out,
                    },
                    cancellation,
                    delivery_deadline,
                )
                .await?;
                plan.finish(job.token, Instant::now());
                active.remove(&event.job_id);
                if early_handoff && plan.initial_completed() && active.is_empty() && has_https_candidate {
                    return Ok(plan.can_resume(Instant::now()).then_some(plan));
                }
            }
            EventPayload::Error(worker) => {
                let reason = match (worker.stage.as_str(), worker.reason.as_str(), worker.retryable) {
                    ("scheduler", "busy", true) => FailureReason::InvalidOutput,
                    ("probe", "probeFailed", false) => FailureReason::ProcessExited,
                    _ => FailureReason::InvalidOutput,
                };
                let failure = WarpFailure::new(FailureStage::Scanner, reason).for_transport(job.kind);
                emit(
                    output,
                    SearchEvent::Failure {
                        kind: job.kind,
                        failure,
                    },
                    cancellation,
                    delivery_deadline,
                )
                .await?;
                return Err(failure.into());
            }
            EventPayload::Accepted | EventPayload::Progress(_) => {}
            EventPayload::Hello(_) => return Err(failure(FailureReason::InvalidOutput)),
        }
    }
}

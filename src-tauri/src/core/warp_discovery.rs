use super::warp_failure::{FailureReason, FailureStage, WarpFailure};
use crate::{
    config::{
        Config,
        warp_profiles::{ServerSelection, WarpCatalog, WarpCatalogLock},
    },
    core::warp_probe::{
        ProbeCancellation, TransportProbeReport, TransportUnavailable, run_warp_transport_probe_profile_timed,
    },
};
use anyhow::{Result, anyhow, ensure};
use std::{path::PathBuf, time::Duration};

pub(crate) use super::warp_search::SearchProgress;
use super::warp_search::SearchState;
use crate::config::warp_profiles::TransportKind;

#[cfg(test)]
#[test]
fn unfinished_checks_are_not_reported_as_rejected_candidates() {
    let mut progress = SearchProgress::default();
    progress.record_check(TransportKind::WireGuard, "one", Err(&anyhow!("WARP probe cancelled")));
    progress.record_check(
        TransportKind::WireGuard,
        "one",
        Err(&anyhow!("Transport core shutdown could not be confirmed")),
    );
    assert_eq!((progress.checked, progress.verified), (0, 0));
    progress.record_check(TransportKind::WireGuard, "one", Err(&anyhow!(TransportUnavailable)));
    assert_eq!((progress.checked, progress.verified), (1, 0));
    progress.record_check(TransportKind::WireGuard, "two", Ok(()));
    assert_eq!((progress.checked, progress.verified), (2, 1));
}

#[cfg(test)]
#[test]
fn search_results_separate_wg_from_awg_and_keep_unfinished_checks_untested() {
    let mut progress = SearchProgress::default();
    progress.preset = "wg".into();
    progress.discover(TransportKind::WireGuard, ["one".into(), "two".into(), "three".into()]);
    progress.record_check(TransportKind::WireGuard, "one", Ok(()));
    progress.record_check(TransportKind::WireGuard, "two", Err(&anyhow!(TransportUnavailable)));
    progress.record_check(
        TransportKind::WireGuard,
        "three",
        Err(&anyhow!("cleanup not confirmed")),
    );
    progress.discover(TransportKind::WireGuard, ["one".into(), "two".into()]);
    progress.record_check(TransportKind::WireGuard, "one", Ok(()));
    let wire = serde_json::to_value(progress).unwrap();
    assert_eq!(wire["transports"][0]["transport"], "wireGuard");
    assert_eq!(wire["transports"][0]["discovered"], 3);
    assert_eq!(wire["transports"][0]["verified"], 1);
    assert_eq!(wire["transports"][0]["rejected"], 1);
    assert_eq!(wire["transports"][0]["untested"], 1);
    assert_eq!(wire["transports"][1]["checked"], 0);
}

fn can_continue_after_scan_failure(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<WarpFailure>()
        .is_some_and(|failure| match failure.stage {
            FailureStage::Registration => {
                failure.reason != FailureReason::Cancelled && failure.reason != FailureReason::CleanupPending
            }
            FailureStage::Scanner => matches!(
                failure.reason,
                FailureReason::Timeout
                    | FailureReason::NoCandidates
                    | FailureReason::ProcessExited
                    | FailureReason::InvalidOutput
                    | FailureReason::Unreachable
            ),
            _ => false,
        })
}

fn observation_time() -> Result<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs())
}

pub(crate) static SEARCH_PROGRESS: std::sync::LazyLock<parking_lot::Mutex<SearchProgress>> =
    std::sync::LazyLock::new(Default::default);

#[cfg(feature = "tropa-probe")]
pub async fn run_warp_discovery_probe(
    root: PathBuf,
    core: PathBuf,
    runtime_root: PathBuf,
) -> Result<TransportProbeReport> {
    run_profile_discovery(root, core, runtime_root, false, &ProbeCancellation::new(), None, false).await
}

#[cfg(feature = "tropa-probe")]
pub async fn refresh_warp_discovery_probe(
    root: PathBuf,
    core: PathBuf,
    runtime_root: PathBuf,
) -> Result<TransportProbeReport> {
    run_profile_discovery(root, core, runtime_root, true, &ProbeCancellation::new(), None, false).await
}

#[cfg(not(windows))]
pub(crate) async fn run_profile_discovery(
    _root: PathBuf,
    _core: PathBuf,
    _runtime_root: PathBuf,
    _refresh: bool,
    _cancellation: &ProbeCancellation,
    _transport: Option<TransportKind>,
    _advanced: bool,
) -> Result<TransportProbeReport> {
    Err(WarpFailure::new(FailureStage::Scanner, FailureReason::Unsupported).into())
}

#[cfg(windows)]
pub(crate) async fn run_profile_discovery(
    root: PathBuf,
    core: PathBuf,
    runtime_root: PathBuf,
    refresh: bool,
    cancellation: &ProbeCancellation,
    transport: Option<TransportKind>,
    advanced: bool,
) -> Result<TransportProbeReport> {
    ensure!(
        refresh || (transport.is_none() && !advanced),
        WarpFailure::new(FailureStage::Scanner, FailureReason::Unsupported)
    );
    ensure!(
        root.is_absolute() && core.is_absolute() && runtime_root.is_absolute(),
        "Probe paths must be absolute"
    );
    let _write = Config::lock_config_write().await;
    let _catalog_write = WarpCatalogLock::acquire(&root)?;
    let path = root.join("warp-profiles.json");
    let mut catalog = WarpCatalog::load_current(&root).await?;
    let mut policy = crate::config::tropa::TropaPreferences::load(&root.join("tropa.yaml"))
        .await?
        .selection_policy
        .for_search(transport)?;
    ensure!(
        !advanced
            || (transport != Some(TransportKind::WireGuard)
                && [TransportKind::Awg, TransportKind::MasqueH3, TransportKind::MasqueH2]
                    .iter()
                    .any(|kind| policy.allows_kind(*kind))),
        WarpFailure::new(FailureStage::Scanner, FailureReason::Unsupported)
    );
    drop(_write);
    if !refresh && matches!(catalog.selection, ServerSelection::Manual { .. }) {
        let context = crate::config::warp_context::capture();
        let selected = catalog.profiles_to_validate(&policy, context.as_deref());
        let selected = selected
            .first()
            .ok_or_else(|| WarpFailure::new(FailureStage::Validation, FailureReason::PolicyRestricted))?;
        policy.allowed_transports = vec![selected.transport.kind()];
    }
    {
        let mut progress = SEARCH_PROGRESS.lock();
        *progress = SearchProgress::default();
        progress.transports.retain(|entry| policy.allows_kind(entry.transport));
    }
    let mut finish_progress = scopeguard::guard(None::<tokio::time::Instant>, |deadline| {
        let mut progress = SEARCH_PROGRESS.lock();
        progress.time_limit_reached = deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline);
        let timed_out = progress.time_limit_reached;
        for entry in &mut progress.transports {
            if entry.state == SearchState::Queued || entry.state == SearchState::Running {
                if cancellation.is_cancelled() {
                    entry.state = SearchState::Cancelled;
                } else if timed_out {
                    entry.state = SearchState::TimedOut;
                }
            }
        }
    });
    ensure!(
        refresh
            || matches!(catalog.selection, ServerSelection::Automatic)
            || !catalog
                .profiles_to_validate(&policy, crate::config::warp_context::capture().as_deref())
                .is_empty(),
        WarpFailure::new(FailureStage::Validation, FailureReason::PolicyRestricted)
    );
    let mut registration_failures = super::warp_search::RegistrationFailures::default();
    {
        let _write = Config::lock_config_write().await;
        use crate::config::warp_profiles::{WarpProfile, WarpTransport};
        let route = std::sync::Arc::new(super::warp_registration_http::RegistrationRoute::for_runtime(
            &runtime_root,
        ));
        for (kinds, endpoint, transport) in [
            (
                [TransportKind::WireGuard, TransportKind::Awg],
                "188.114.98.58:2408",
                WarpTransport::WireGuard,
            ),
            (
                [TransportKind::MasqueH3, TransportKind::MasqueH2],
                "162.159.198.2:443",
                WarpTransport::MasqueH3 {
                    sni: "consumer-masque.cloudflareclient.com".into(),
                },
            ),
        ] {
            if !kinds.iter().any(|kind| policy.allows_kind(*kind)) {
                continue;
            }
            let profile = WarpProfile::new(endpoint.parse()?, transport)?;
            let prepared = tokio::select! {
                biased;
                _ = cancellation.cancelled() => return Err(WarpFailure::new(FailureStage::Registration, FailureReason::Cancelled).into()),
                result = tokio::time::timeout(Duration::from_secs(110),
                    super::warp_runtime::RuntimeAccount::ensure_with_route(&root, &profile, std::sync::Arc::clone(&route))) => {
                    result.map_err(|_| anyhow::Error::from(WarpFailure::new(FailureStage::Registration, FailureReason::Timeout)))
                        .and_then(|result| result)
                }
            };
            if let Err(error) = prepared {
                let failure = WarpFailure::from_error(&error, FailureStage::Registration, FailureReason::InvalidOutput);
                for kind in kinds.into_iter().filter(|kind| policy.allows_kind(*kind)) {
                    registration_failures.remember(kind, failure);
                    SEARCH_PROGRESS.lock().fail(kind, failure.for_transport(kind));
                }
            }
        }
    }
    let allowed: Vec<_> = [
        TransportKind::WireGuard,
        TransportKind::Awg,
        TransportKind::MasqueH3,
        TransportKind::MasqueH2,
    ]
    .into_iter()
    .filter(|kind| policy.allows_kind(*kind) && registration_failures.failure(*kind).is_none())
    .collect();
    if allowed.is_empty() {
        return Err([TransportKind::WireGuard, TransportKind::MasqueH3]
            .into_iter()
            .find_map(|kind| registration_failures.failure(kind))
            .expect("failed registration family")
            .into());
    }
    policy.allowed_transports = allowed;
    let started = tokio::time::Instant::now();
    let deadline = started + Duration::from_secs(if refresh { 600 } else { 180 });
    *finish_progress = Some(deadline);
    let mut last_scan_failure = None;
    if !refresh {
        let context = crate::config::warp_context::capture();
        let cached: Vec<_> = catalog
            .profiles_to_validate(&policy, context.as_deref())
            .into_iter()
            .take(3)
            .collect();
        ensure!(
            !cached.is_empty() || matches!(catalog.selection, ServerSelection::Automatic),
            WarpFailure::new(FailureStage::Validation, FailureReason::PolicyRestricted)
        );
        for profile in &cached {
            SEARCH_PROGRESS
                .lock()
                .discover(profile.transport.kind(), [profile.id.clone()]);
        }
        for profile in cached {
            if let Some(failure) = registration_failures.failure(profile.transport.kind()) {
                SEARCH_PROGRESS.lock().fail(profile.transport.kind(), failure);
                continue;
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            eprintln!(
                "Revalidating cached {} profile at {}",
                profile.transport.label(),
                profile.endpoint
            );
            let kind = profile.transport.kind();
            {
                let mut progress = SEARCH_PROGRESS.lock();
                progress.preset = "cached".into();
                progress.begin_task(kind);
            }
            let cached_complete = std::sync::atomic::AtomicBool::new(false);
            let _cached_progress = scopeguard::guard(tokio::time::Instant::now(), |started| {
                let mut progress = SEARCH_PROGRESS.lock();
                let entry = progress.transport(kind);
                entry.elapsed_ms = entry.elapsed_ms.saturating_add(started.elapsed().as_millis() as u64);
                if entry.state == SearchState::Running {
                    entry.state = if cancellation.is_cancelled() {
                        SearchState::Cancelled
                    } else if cached_complete.load(std::sync::atomic::Ordering::Relaxed) {
                        SearchState::Completed
                    } else {
                        SearchState::Failed
                    };
                }
            });
            let cached_deadline = if matches!(catalog.selection, ServerSelection::Manual { .. }) {
                deadline
            } else {
                deadline.min(tokio::time::Instant::now() + Duration::from_secs(15))
            };
            let cached_cancellation = ProbeCancellation::linked(cancellation, cached_deadline);
            let timings = SEARCH_PROGRESS.lock().transport(kind).timings.clone();
            match run_warp_transport_probe_profile_timed(
                root.clone(),
                core.clone(),
                &profile,
                &cached_cancellation,
                &timings,
            )
            .await
            {
                Ok(report) => {
                    ensure!(
                        !cancellation.is_cancelled(),
                        WarpFailure::new(FailureStage::Scanner, FailureReason::Cancelled)
                    );
                    let _write = Config::lock_config_write().await;
                    super::warp_runtime::RuntimeAccount::load(&root, &profile)
                        .await?
                        .bind_catalog(&mut catalog);
                    if !policy.allows_observation(&profile.transport, &report.observation, false) {
                        catalog.record_rejection(&profile.id, observation_time()?)?;
                        catalog.save(&path).await?;
                        SEARCH_PROGRESS
                            .lock()
                            .record_check(kind, &profile.id, Err(&anyhow!(TransportUnavailable)));
                        ensure!(
                            matches!(catalog.selection, ServerSelection::Automatic),
                            WarpFailure::new(FailureStage::Validation, FailureReason::PolicyRestricted)
                        );
                        cached_complete.store(true, std::sync::atomic::Ordering::Relaxed);
                        continue;
                    }
                    catalog.record_probe(profile.clone(), report.observation.clone())?;
                    catalog.save(&path).await?;
                    SEARCH_PROGRESS.lock().record_check(kind, &profile.id, Ok(()));
                    cached_complete.store(true, std::sync::atomic::Ordering::Relaxed);
                    return Ok(report);
                }
                Err(error)
                    if !cancellation.is_cancelled()
                        && error
                            .downcast_ref::<WarpFailure>()
                            .is_some_and(|failure| failure.reason == FailureReason::Cancelled)
                        && tokio::time::Instant::now() >= cached_deadline =>
                {
                    let failure =
                        WarpFailure::new(FailureStage::Validation, FailureReason::Timeout).for_transport(kind);
                    SEARCH_PROGRESS.lock().fail(kind, failure);
                    if matches!(catalog.selection, ServerSelection::Automatic) {
                        cached_complete.store(true, std::sync::atomic::Ordering::Relaxed);
                        continue;
                    }
                    return Err(failure.into());
                }
                Err(error) if error.downcast_ref::<TransportUnavailable>().is_some() => {
                    let _write = Config::lock_config_write().await;
                    super::warp_runtime::RuntimeAccount::load(&root, &profile)
                        .await?
                        .bind_catalog(&mut catalog);
                    catalog.record_rejection(&profile.id, observation_time()?)?;
                    catalog.save(&path).await?;
                    SEARCH_PROGRESS.lock().record_check(kind, &profile.id, Err(&error));
                    cached_complete.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                Err(error) => {
                    let failure =
                        WarpFailure::from_error(&error, FailureStage::Validation, FailureReason::InvalidOutput)
                            .for_transport(kind);
                    SEARCH_PROGRESS.lock().fail(kind, failure);
                    if matches!(catalog.selection, ServerSelection::Automatic)
                        && can_continue_after_scan_failure(&error)
                    {
                        registration_failures.remember(kind, failure);
                        last_scan_failure = Some(failure);
                        cached_complete.store(true, std::sync::atomic::Ordering::Relaxed);
                        continue;
                    }
                    return Err(error);
                }
            }
        }
        ensure!(
            matches!(catalog.selection, ServerSelection::Automatic),
            WarpFailure::new(FailureStage::Validation, FailureReason::Unreachable)
        );
    }
    let mut resume = None;
    let mut search_pending = true;
    let mut baselines = Vec::new();
    let mut attempted = std::collections::HashSet::new();
    let mut found: Vec<(String, TransportProbeReport)> = Vec::new();
    loop {
        if search_pending {
            let collected = super::warp_engine::stream::search(
                &root,
                &runtime_root,
                &mut catalog,
                &policy,
                crate::config::warp_context::ipv6_scan_available(),
                refresh,
                advanced,
                started,
                observation_time()?,
                cancellation,
                &SEARCH_PROGRESS,
                resume.take(),
            )
            .await?;
            last_scan_failure = collected.last_failure.or(last_scan_failure);
            baselines.extend(collected.baselines);
            resume = collected
                .resume
                .filter(|plan| plan.can_resume(tokio::time::Instant::now()));
        }
        let validation_deadline = round_validation_deadline(
            tokio::time::Instant::now(),
            deadline,
            resume.as_ref().map(|plan| plan.discovery_deadline()),
        );
        let context = crate::config::warp_context::capture();
        let mut validation_catalog = catalog.clone();
        validation_catalog.selection = ServerSelection::Automatic;
        let profiles: Vec<_> = super::warp_engine::selection::validation_queue(
            &baselines,
            &validation_catalog,
            &policy,
            context.as_deref(),
            observation_time()?,
        )
        .into_iter()
        .filter(|profile| !attempted.contains(&profile.id))
        .collect();
        for (index, profile) in profiles.iter().enumerate() {
            ensure!(
                !cancellation.is_cancelled(),
                WarpFailure::new(FailureStage::Validation, FailureReason::Cancelled)
            );
            let now = tokio::time::Instant::now();
            if now >= validation_deadline {
                break;
            }
            let remaining_kinds = [
                TransportKind::WireGuard,
                TransportKind::Awg,
                TransportKind::MasqueH3,
                TransportKind::MasqueH2,
            ]
            .into_iter()
            .filter(|kind| {
                profiles[index..]
                    .iter()
                    .any(|profile| profile.transport.kind() == *kind)
            })
            .count();
            let budget = validation_deadline.saturating_duration_since(now) / remaining_kinds.max(1) as u32;
            if budget < Duration::from_secs(2) {
                break;
            }
            let slice_deadline =
                validation_deadline.min(now + budget.min(Duration::from_secs(if refresh { 30 } else { 20 })));
            let slice_cancellation = ProbeCancellation::linked(cancellation, slice_deadline);
            let kind = profile.transport.kind();
            attempted.insert(profile.id.clone());
            let timings = {
                let mut progress = SEARCH_PROGRESS.lock();
                progress.preset = "mihomo".into();
                progress.begin_task(kind);
                progress.discover(kind, [profile.id.clone()]);
                progress.transport(kind).timings.clone()
            };
            let result = run_warp_transport_probe_profile_timed(
                root.clone(),
                core.clone(),
                profile,
                &slice_cancellation,
                &timings,
            )
            .await;
            {
                let mut progress = SEARCH_PROGRESS.lock();
                let entry = progress.transport(kind);
                entry.elapsed_ms = entry.elapsed_ms.saturating_add(now.elapsed().as_millis() as u64);
                entry.state = if cancellation.is_cancelled() {
                    SearchState::Cancelled
                } else if slice_cancellation.is_cancelled() {
                    SearchState::TimedOut
                } else {
                    SearchState::Completed
                };
            }
            if slice_cancellation.is_cancelled()
                && !cancellation.is_cancelled()
                && result
                    .as_ref()
                    .err()
                    .and_then(|error| error.downcast_ref::<WarpFailure>())
                    .is_some_and(|failure| failure.reason == FailureReason::Cancelled)
            {
                last_scan_failure =
                    Some(WarpFailure::new(FailureStage::Validation, FailureReason::Timeout).for_transport(kind));
                continue;
            }
            match result {
                Ok(mut report) => {
                    ensure!(
                        !cancellation.is_cancelled(),
                        WarpFailure::new(FailureStage::Validation, FailureReason::Cancelled)
                    );
                    let _write = Config::lock_config_write().await;
                    super::warp_runtime::RuntimeAccount::load(&root, profile)
                        .await?
                        .bind_catalog(&mut catalog);
                    if !policy.allows_observation(&profile.transport, &report.observation, false) {
                        if catalog.entries.iter().any(|entry| entry.profile.id == profile.id)
                            || catalog.scanned.iter().any(|entry| entry.profile.id == profile.id)
                        {
                            catalog.record_rejection(&profile.id, observation_time()?)?;
                            catalog.save(&path).await?;
                        }
                        SEARCH_PROGRESS
                            .lock()
                            .record_check(kind, &profile.id, Err(&anyhow!(TransportUnavailable)));
                        continue;
                    }
                    catalog.record_probe(profile.clone(), report.observation.clone())?;
                    catalog.save(&path).await?;
                    report.observation = catalog
                        .entries
                        .iter()
                        .find(|entry| entry.profile.id == profile.id)
                        .expect("recorded runtime observation")
                        .observation
                        .clone();
                    SEARCH_PROGRESS.lock().record_check(kind, &profile.id, Ok(()));
                    found.push((profile.id.clone(), report));
                    if !refresh {
                        break;
                    }
                }
                Err(error) if error.downcast_ref::<TransportUnavailable>().is_some() => {
                    let _write = Config::lock_config_write().await;
                    if catalog.entries.iter().any(|entry| entry.profile.id == profile.id)
                        || catalog.scanned.iter().any(|entry| entry.profile.id == profile.id)
                    {
                        catalog.record_rejection(&profile.id, observation_time()?)?;
                        catalog.save(&path).await?;
                    }
                    SEARCH_PROGRESS.lock().record_check(kind, &profile.id, Err(&error));
                }
                Err(error) => {
                    SEARCH_PROGRESS.lock().fail(
                        kind,
                        WarpFailure::from_error(&error, FailureStage::Validation, FailureReason::InvalidOutput)
                            .for_transport(kind),
                    );
                    return Err(error);
                }
            }
        }
        if !found.is_empty() {
            break;
        }
        let Some(next) = resume.take() else {
            break;
        };
        search_pending = next.can_resume(tokio::time::Instant::now());
        if search_pending {
            resume = Some(next);
        }
    }
    // A refresh reports the best new observation without changing a manual selection.
    catalog.selection = ServerSelection::Automatic;
    let best = catalog
        .candidates()
        .into_iter()
        .find(|entry| !catalog.is_rejected(&entry.profile.id) && found.iter().any(|(id, _)| *id == entry.profile.id));
    let best_id = best.map(|entry| entry.profile.id.clone());
    found
        .into_iter()
        .find(|(id, _)| Some(id) == best_id.as_ref())
        .map(|(_, report)| report)
        .ok_or_else(|| {
            if let Some(failure) = last_scan_failure {
                return failure.into();
            }
            WarpFailure::new(
                if SEARCH_PROGRESS.lock().discovered == 0 {
                    FailureStage::Scanner
                } else {
                    FailureStage::Validation
                },
                if tokio::time::Instant::now() >= deadline {
                    FailureReason::Timeout
                } else {
                    FailureReason::NoCandidates
                },
            )
            .into()
        })
}

fn round_validation_deadline(
    now: tokio::time::Instant,
    deadline: tokio::time::Instant,
    continuation_deadline: Option<tokio::time::Instant>,
) -> tokio::time::Instant {
    continuation_deadline
        .filter(|discovery| *discovery > now)
        .map_or(deadline, |discovery| {
            deadline.min(discovery).min(now + Duration::from_secs(20))
        })
}

#[cfg(test)]
#[test]
fn early_validation_leaves_time_for_the_same_search_to_continue() {
    let start = tokio::time::Instant::now();
    let now = start + Duration::from_secs(30);
    let deadline = start + Duration::from_secs(180);
    assert_eq!(
        round_validation_deadline(now, deadline, Some(start + Duration::from_secs(135))),
        start + Duration::from_secs(50),
    );
    assert_eq!(
        round_validation_deadline(now, deadline, Some(start + Duration::from_secs(40))),
        start + Duration::from_secs(40),
    );
    assert_eq!(round_validation_deadline(now, deadline, None), deadline);
    assert_eq!(
        round_validation_deadline(
            start + Duration::from_secs(136),
            deadline,
            Some(start + Duration::from_secs(135))
        ),
        deadline,
    );
}

#[cfg(all(test, windows))]
#[tokio::test]
async fn cancellation_during_initial_registration_finalizes_search_progress() -> Result<()> {
    let root = std::env::temp_dir().join(format!("tropa-cancel-registration-{}", nanoid::nanoid!()));
    tokio::fs::create_dir(&root).await?;
    let cleanup = scopeguard::guard(root, |path| {
        let _ = std::fs::remove_dir_all(path);
    });
    let cancellation = ProbeCancellation::new();
    cancellation.cancel_handle().send(true)?;
    let result = run_profile_discovery(
        cleanup.clone(),
        cleanup.join("resources/core.exe"),
        cleanup.clone(),
        false,
        &cancellation,
        None,
        false,
    )
    .await;
    assert!(result.err().is_some_and(|error| {
        error
            .downcast_ref::<WarpFailure>()
            .is_some_and(|failure| failure.reason == FailureReason::Cancelled)
    }));
    assert!(
        SEARCH_PROGRESS
            .lock()
            .transports
            .iter()
            .all(|entry| entry.state == SearchState::Cancelled)
    );
    assert!(!cleanup.join("secrets/warp-account.json").exists());
    Ok(())
}

#[cfg(test)]
#[test]
fn family_registration_errors_do_not_abort_other_modes_but_cleanup_and_cancel_do() {
    for stage in [FailureStage::Registration, FailureStage::Scanner] {
        for reason in [
            FailureReason::Timeout,
            FailureReason::InvalidOutput,
            FailureReason::Unreachable,
        ] {
            assert!(can_continue_after_scan_failure(&anyhow!(WarpFailure::new(
                stage, reason
            ))));
        }
    }
    for failure in [
        WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending),
        WarpFailure::new(FailureStage::Registration, FailureReason::Cancelled),
        WarpFailure::new(FailureStage::Scanner, FailureReason::Cancelled),
        WarpFailure::new(FailureStage::Runtime, FailureReason::ProcessExited),
    ] {
        assert!(!can_continue_after_scan_failure(&anyhow!(failure)));
    }
    assert!(!can_continue_after_scan_failure(&anyhow!("ownership not reserved")));
}

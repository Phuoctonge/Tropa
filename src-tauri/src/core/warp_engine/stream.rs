use super::runner::SearchEvent;
use crate::core::{
    warp_failure::{FailureReason, FailureStage},
    warp_probe::ProbeCancellation,
};
use crate::{
    config::{
        Config,
        warp_profiles::{RegistrationFamily, TransportKind, WarpCatalog, WarpProfile},
        warp_selection::SelectionPolicy,
    },
    core::{
        warp_failure::WarpFailure,
        warp_runtime::RuntimeAccount,
        warp_search::{SearchProgress, SearchState},
    },
};
use anyhow::Result;
use std::path::Path;

pub(crate) async fn search(
    root: &Path,
    runtime_root: &Path,
    catalog: &mut WarpCatalog,
    policy: &SelectionPolicy,
    ipv6: bool,
    extended: bool,
    start: tokio::time::Instant,
    seed: u64,
    cancellation: &ProbeCancellation,
    progress: &parking_lot::Mutex<SearchProgress>,
) -> Result<CollectedSearch> {
    let lifetime = ProbeCancellation::linked(
        cancellation,
        start + std::time::Duration::from_secs(if extended { 600 } else { 180 }),
    );
    let (sender, receiver) = tokio::sync::mpsc::channel(32);
    let producer_lifetime = &lifetime;
    let producer = async move {
        let result = super::runner::run_search(
            root,
            runtime_root,
            policy,
            ipv6,
            extended,
            start,
            seed,
            producer_lifetime,
            &sender,
        )
        .await;
        drop(sender);
        result
    };
    let consumer = async {
        let result = collect(root, catalog, policy, progress, receiver).await;
        if result.is_err() {
            let _ = lifetime.cancel_handle().send(true);
        }
        result
    };
    // Both futures must finish: dropping the consumer cannot authorize runtime ownership.
    let (produced, consumed) = tokio::join!(producer, consumer);
    if let Err(error) = &produced {
        if error
            .downcast_ref::<WarpFailure>()
            .is_some_and(|failure| failure.reason == FailureReason::CleanupPending)
        {
            return produced.and(consumed);
        }
    }
    let mut collected = consumed?;
    if let Err(error) = produced {
        let failure = WarpFailure::from_error(&error, FailureStage::Scanner, FailureReason::InvalidOutput);
        if failure.reason != FailureReason::Timeout || cancellation.is_cancelled() {
            return Err(error);
        }
        collected.last_failure = Some(failure);
    }
    Ok(collected)
}

#[derive(Default)]
pub(crate) struct CollectedSearch {
    pub(crate) baselines: Vec<WarpProfile>,
    pub(crate) last_failure: Option<WarpFailure>,
}

fn slot(kind: TransportKind) -> usize {
    match kind {
        TransportKind::WireGuard => 0,
        TransportKind::Awg => 1,
        TransportKind::MasqueH3 => 2,
        TransportKind::MasqueH2 => 3,
    }
}

async fn bind_account(
    root: &Path,
    catalog: &mut WarpCatalog,
    profile: &WarpProfile,
    bound: &mut [bool; 2],
) -> Result<()> {
    let index = match profile.transport.family() {
        RegistrationFamily::WireGuard => 0,
        RegistrationFamily::Masque => 1,
    };
    if !bound[index] {
        RuntimeAccount::load(root, profile).await?.bind_catalog(catalog);
        bound[index] = true;
    }
    Ok(())
}

pub(crate) async fn collect(
    root: &Path,
    catalog: &mut WarpCatalog,
    policy: &SelectionPolicy,
    progress: &parking_lot::Mutex<SearchProgress>,
    mut events: tokio::sync::mpsc::Receiver<SearchEvent>,
) -> Result<CollectedSearch> {
    let mut collected = CollectedSearch::default();
    let mut bound = [false; 2];
    let mut active = [0usize; 4];
    while let Some(event) = events.recv().await {
        match event {
            SearchEvent::Prepared(profiles) => {
                let _write = Config::lock_config_write().await;
                for profile in &profiles {
                    bind_account(root, catalog, profile, &mut bound).await?;
                }
                catalog.save(&root.join("warp-profiles.json")).await?;
                collected.baselines = profiles;
            }
            SearchEvent::Unavailable(value) => progress.lock().unavailable_tuning.push(value),
            SearchEvent::Started {
                kind,
                parameter_name,
                ipv6,
            } => {
                active[slot(kind)] += 1;
                let mut progress = progress.lock();
                progress.preset = parameter_name;
                progress.address_family = Some(if ipv6 { "ipv6" } else { "ipv4" });
                progress.begin_task(kind);
            }
            SearchEvent::Candidate(entry) => {
                if !policy.allows_transport(&entry.profile.transport)
                    || !policy.matches_metadata(
                        entry.observation.node.as_deref(),
                        entry.observation.node_country.as_deref(),
                        true,
                    )
                {
                    continue;
                }
                let kind = entry.profile.transport.kind();
                let id = entry.profile.id.clone();
                let _write = Config::lock_config_write().await;
                bind_account(root, catalog, &entry.profile, &mut bound).await?;
                catalog.record_scanned(entry.profile, entry.observation)?;
                catalog.save(&root.join("warp-profiles.json")).await?;
                progress.lock().discover(kind, [id]);
            }
            SearchEvent::Finished {
                kind,
                elapsed_ms,
                timed_out,
            } => {
                active[slot(kind)] = active[slot(kind)].saturating_sub(1);
                let mut progress = progress.lock();
                let entry = progress.transport(kind);
                entry.elapsed_ms = entry.elapsed_ms.saturating_add(elapsed_ms);
                if active[slot(kind)] == 0 {
                    entry.state = if timed_out {
                        SearchState::TimedOut
                    } else {
                        SearchState::Completed
                    };
                }
            }
            SearchEvent::Failure { kind, failure } => {
                progress.lock().fail(kind, failure);
                collected.last_failure = Some(failure);
            }
        }
    }
    Ok(collected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{
            warp_account::{WarpAccount, WarpAccountStore},
            warp_profiles::{ServerSelection, WarpCatalog},
        },
        core::warp_engine::wire::Candidate,
    };
    use base64::{Engine, engine::general_purpose::STANDARD};

    #[tokio::test]
    async fn failed_worker_start_closes_the_stream_without_waiting_for_events() -> Result<()> {
        let missing = std::env::temp_dir().join(format!("tropa-missing-worker-{}", nanoid::nanoid!()));
        let mut catalog = WarpCatalog::default();
        let progress = parking_lot::Mutex::new(SearchProgress::default());
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            search(
                &missing,
                &missing,
                &mut catalog,
                &SelectionPolicy::default(),
                false,
                false,
                tokio::time::Instant::now(),
                1,
                &ProbeCancellation::new(),
                &progress,
            ),
        )
        .await?;
        assert!(result.is_err());
        assert!(!missing.exists());
        Ok(())
    }

    #[tokio::test]
    async fn streamed_candidates_are_saved_before_terminal_without_secrets_or_false_verification() -> anyhow::Result<()>
    {
        let root = std::env::temp_dir().join(format!("tropa-stream-{}", nanoid::nanoid!()));
        std::fs::create_dir(&root)?;
        let _directory = scopeguard::guard(root.clone(), |path| {
            let _ = std::fs::remove_dir_all(path);
        });
        let account = WarpAccount::from_warpscout_json(&serde_json::to_vec(&serde_json::json!({
            "id":"synthetic-stream", "token":"stream-secret-marker",
            "private_key":STANDARD.encode([7u8;32]), "peer_public_key":STANDARD.encode([9u8;32]),
            "ipv4":"172.16.0.2", "ipv6":null
        }))?)?;
        WarpAccountStore::new(root.clone()).save(&account).await?;
        let candidate: Candidate = serde_json::from_value(serde_json::json!({
            "endpoint":"188.114.98.58:2408","transport":"wg","parameters":{},
            "stages":{"handshake":true,"http":true,"https":false,"udp":null},
            "endpointPingMs":1.25,"tunnelPingMs":null,"lossPercent":null,"jitterMs":null,
            "samples":0,"method":"","node":"HEL","nodeCountry":"FI","exitCountry":null
        }))?;
        let scanned = candidate.into_scanned(format!("source:{}", "b".repeat(64)), Some("a".repeat(64)), 123)?;
        let mut excluded = scanned.clone();
        excluded.profile =
            crate::config::warp_profiles::WarpProfile::new("188.114.96.10:500".parse()?, excluded.profile.transport)?;
        excluded.observation.node = Some("DME".into());
        let profile = scanned.profile.clone();
        let mut catalog = WarpCatalog::default();
        catalog.record_scanned(profile.clone(), scanned.observation.clone())?;
        catalog.select(ServerSelection::Manual {
            profile_id: profile.id.clone(),
        })?;
        let selection = catalog.selection.clone();
        let policy = SelectionPolicy {
            exclude_nodes: vec!["DME".into()],
            ..Default::default()
        };
        let progress = parking_lot::Mutex::new(SearchProgress::default());
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        let producer = async {
            sender
                .send(SearchEvent::Started {
                    kind: TransportKind::WireGuard,
                    parameter_name: "wg".into(),
                    ipv6: false,
                })
                .await
                .map_err(|_| anyhow::anyhow!("consumer closed"))?;
            sender
                .send(SearchEvent::Candidate(scanned))
                .await
                .map_err(|_| anyhow::anyhow!("consumer closed"))?;
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                loop {
                    if let Ok(bytes) = tokio::fs::read(root.join("warp-profiles.json")).await {
                        let disk: WarpCatalog = serde_json::from_slice(&bytes)?;
                        if disk.scanned.len() == 1 {
                            assert!(disk.entries.is_empty());
                            assert!(!String::from_utf8_lossy(&bytes).contains("stream-secret-marker"));
                            break;
                        }
                    }
                    tokio::task::yield_now().await;
                }
                Ok::<_, anyhow::Error>(())
            })
            .await??;
            sender
                .send(SearchEvent::Candidate(excluded))
                .await
                .map_err(|_| anyhow::anyhow!("consumer closed"))?;
            sender
                .send(SearchEvent::Finished {
                    kind: TransportKind::WireGuard,
                    elapsed_ms: 1234,
                    timed_out: false,
                })
                .await
                .map_err(|_| anyhow::anyhow!("consumer closed"))?;
            drop(sender);
            Ok::<_, anyhow::Error>(())
        };
        let (received, sent) = tokio::join!(collect(&root, &mut catalog, &policy, &progress, receiver), producer);
        sent?;
        received?;
        assert_eq!(catalog.selection, selection);
        assert_eq!(catalog.scanned.len(), 1);
        let view = serde_json::to_value(progress.into_inner())?;
        assert_eq!(view["discovered"], 1);
        assert_eq!(view["verified"], 0);
        assert_eq!(view["transports"][0]["state"], "completed");
        assert_eq!(view["transports"][0]["elapsedMs"], 1234);
        Ok(())
    }
}

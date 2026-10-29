use super::wire::{self, Command, EngineEvent, EventPayload, EventTracker, ProbeRequest};
use crate::core::{
    warp_failure::{FailureReason, FailureStage, WarpFailure},
    warp_probe::ProbeCancellation,
};
use anyhow::{Result, anyhow, ensure};
use sha2::{Digest, Sha256};
use std::{
    os::windows::io::OwnedHandle,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    process::{Child, ChildStdin},
    sync::mpsc,
    task::JoinHandle,
    time::Instant,
};

pub(crate) struct WorkerArtifact {
    path: PathBuf,
    sha256: String,
    build_id: String,
}

impl WorkerArtifact {
    pub(crate) fn packaged(root: &Path) -> Result<Self> {
        let pin = option_env!("TROPA_ENGINE_PIN").ok_or_else(|| failure(FailureReason::Unsupported))?;
        Self::from_pin(root, pin)
    }

    fn from_pin(root: &Path, encoded: &str) -> Result<Self> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Pin {
            protocol_version: u32,
            build_id: String,
            snapshot_id: String,
            sha256: String,
        }
        let pin: Pin = serde_json::from_str(encoded).map_err(|_| anyhow!("Invalid compiled worker binding"))?;
        let hex = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        ensure!(
            pin.protocol_version == wire::VERSION
                && hex(&pin.snapshot_id)
                && hex(&pin.sha256)
                && pin.build_id == format!("source:{}", pin.snapshot_id),
            "Invalid compiled worker source binding"
        );
        Ok(Self {
            path: root.join("resources/tropa-probe.exe"),
            sha256: pin.sha256,
            build_id: pin.build_id,
        })
    }

    async fn verify(&self) -> Result<()> {
        ensure!(
            self.sha256.len() == 64
                && self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
                && !self.build_id.is_empty(),
            "Invalid worker build binding"
        );
        let mut file = tokio::fs::File::open(&self.path)
            .await
            .map_err(|_| anyhow!("Unable to read pinned worker"))?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 16384];
        loop {
            let size = file
                .read(&mut buffer)
                .await
                .map_err(|_| anyhow!("Unable to hash pinned worker"))?;
            if size == 0 {
                break;
            }
            hash.update(&buffer[..size]);
        }
        let actual: String = hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
        ensure!(actual == self.sha256, "Worker hash differs from its build binding");
        Ok(())
    }
}

pub(crate) struct EngineClient {
    child: Child,
    input: Option<ChildStdin>,
    events: mpsc::Receiver<Result<EngineEvent>>,
    reader: JoinHandle<()>,
    tracker: EventTracker,
    search_id: String,
    control_job: String,
    cancellation: ProbeCancellation,
    poisoned: bool,
    job: Option<OwnedHandle>,
    _execution: Arc<clash_verge_service_ipc::execution::CoreExecutionGuard>,
}

fn failure(reason: FailureReason) -> anyhow::Error {
    WarpFailure::new(FailureStage::Scanner, reason).into()
}

impl EngineClient {
    pub(crate) fn build_id(&self) -> &str {
        self.tracker.build_id()
    }

    pub(crate) fn parameter_sets(&self) -> &[wire::ParameterSet] {
        self.tracker.parameter_sets()
    }

    pub(crate) fn unavailable_tuning(&self) -> &[wire::UnavailableTuning] {
        self.tracker.unavailable_tuning()
    }
    pub(crate) async fn start(root: &Path, artifact: WorkerArtifact, cancellation: ProbeCancellation) -> Result<Self> {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(failure(FailureReason::Cancelled)),
            result = artifact.verify() => result?,
        }
        let execution = Arc::new(tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(failure(FailureReason::Cancelled)),
            result = crate::core::service::reserve_sidecar() => result?,
        });
        ensure!(!cancellation.is_cancelled(), failure(FailureReason::Cancelled));
        let mut child = tokio::process::Command::new(&artifact.path)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| {
                WarpFailure::from_error(&error.into(), FailureStage::Scanner, FailureReason::ProcessExited)
            })?;
        let pid = child.id().expect("spawned worker PID");
        crate::core::warp_children::track(pid, Arc::clone(&execution));
        let input = child.stdin.take();
        let mut output = child.stdout.take().expect("piped worker output");
        let (sender, events) = mpsc::channel(128);
        let reader = tokio::spawn(async move {
            loop {
                let event = match wire::read_frame(&mut output).await {
                    Ok(data) => wire::decode_event(&data),
                    Err(error) => Err(error),
                };
                let terminal = event.is_err();
                if sender.send(event).await.is_err() || terminal {
                    break;
                }
            }
        });
        let search_id = nanoid::nanoid!();
        let control_job = nanoid::nanoid!();
        let mut client = Self {
            child,
            input,
            events,
            reader,
            tracker: EventTracker::new(search_id.clone(), control_job.clone(), artifact.build_id),
            search_id,
            control_job,
            cancellation,
            poisoned: false,
            job: None,
            _execution: execution,
        };
        let initialized = async {
            client.job = Some(
                crate::core::manager::create_and_assign_sidecar_job(pid)
                    .map_err(|_| anyhow!("Unable to protect worker with a Windows Job Object"))?,
            );
            let control = client.control_job.clone();
            client.send(&control, Command::Hello).await?;
            let hello = tokio::time::timeout(Duration::from_secs(5), client.next_event())
                .await
                .map_err(|_| failure(FailureReason::Timeout))??;
            ensure!(
                matches!(hello.payload, EventPayload::Hello(_)),
                "Missing worker handshake"
            );
            Ok::<_, anyhow::Error>(())
        }
        .await;
        if let Err(error) = initialized {
            client.shutdown().await?;
            return Err(error);
        }
        Ok(client)
    }

    async fn send(&mut self, job: &str, command: Command<'_>) -> Result<()> {
        ensure!(!self.poisoned, failure(FailureReason::InvalidOutput));
        let input = self
            .input
            .as_mut()
            .ok_or_else(|| failure(FailureReason::ProcessExited))?;
        // Dropping this future can leave a partial frame in the pipe.
        self.poisoned = true;
        let result = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(failure(FailureReason::Cancelled)),
            result = tokio::time::timeout(Duration::from_secs(2), wire::write_command(input, &self.search_id, job, command)) => {
                result.map_err(|_| failure(FailureReason::Timeout)).and_then(|result| result)
            },
        };
        self.poisoned = result.is_err();
        result
    }

    pub(crate) async fn submit(&mut self, request: &ProbeRequest) -> Result<String> {
        ensure!(!self.poisoned, failure(FailureReason::InvalidOutput));
        let job = nanoid::nanoid!();
        self.tracker.register_job(&job, request)?;
        self.send(&job, Command::Probe(request)).await?;
        Ok(job)
    }

    pub(crate) async fn cancel(&mut self, job: &str) -> Result<()> {
        self.send(job, Command::Cancel).await
    }

    pub(crate) async fn next_event(&mut self) -> Result<EngineEvent> {
        ensure!(!self.poisoned, failure(FailureReason::InvalidOutput));
        let result = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(failure(FailureReason::Cancelled)),
            result = self.events.recv() => result.ok_or_else(|| failure(FailureReason::ProcessExited)).and_then(|result| result),
        };
        match result.and_then(|event| {
            self.tracker.accept(&event)?;
            Ok(event)
        }) {
            Ok(event) => Ok(event),
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }

    async fn wait_draining(&mut self, deadline: Instant) -> Result<ExitStatus> {
        let mut open = true;
        loop {
            tokio::select! {
                biased;
                result = self.child.wait() => return result.map_err(|_| failure(FailureReason::ProcessExited)),
                _ = tokio::time::sleep_until(deadline) => return Err(failure(FailureReason::Timeout)),
                event = self.events.recv(), if open => { open = event.is_some(); },
            }
        }
    }

    pub(crate) async fn shutdown(mut self) -> Result<()> {
        let graceful_deadline = Instant::now() + Duration::from_secs(2);
        if !self.poisoned {
            if let Some(input) = self.input.as_mut() {
                let _ = tokio::time::timeout_at(
                    graceful_deadline,
                    wire::write_command(input, &self.search_id, &self.control_job, Command::Shutdown),
                )
                .await;
            }
        }
        self.input.take();
        let graceful = self.wait_draining(graceful_deadline).await;
        let status = match graceful {
            Ok(status) => status,
            Err(_) => {
                self.job.take();
                let _ = self.child.start_kill();
                self.wait_draining(Instant::now() + Duration::from_secs(5))
                    .await
                    .map_err(|_| WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending))?
            }
        };
        self.reader.abort();
        self.job.take();
        crate::core::warp_children::wait_for_exit()
            .await
            .map_err(|_| WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending))?;
        ensure!(status.success() || self.poisoned, failure(FailureReason::ProcessExited));
        Ok(())
    }
}

impl Drop for EngineClient {
    fn drop(&mut self) {
        self.reader.abort();
        self.input.take();
        self.job.take();
        let _ = self.child.start_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::super::wire::{Identity, Outcome, Parameters, Transport, WgIdentity};
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    #[test]
    fn runtime_pin_rejects_an_unrelated_source_identity() {
        let mut pin = serde_json::json!({
            "protocolVersion": 1, "buildId": format!("source:{}", "a".repeat(64)),
            "snapshotId": "a".repeat(64), "sha256": "b".repeat(64),
        });
        assert!(WorkerArtifact::from_pin(Path::new("C:/Tropa"), &pin.to_string()).is_ok());
        pin["snapshotId"] = "c".repeat(64).into();
        assert!(WorkerArtifact::from_pin(Path::new("C:/Tropa"), &pin.to_string()).is_err());
        pin["buildId"] = "development".into();
        assert!(WorkerArtifact::from_pin(Path::new("C:/Tropa"), &pin.to_string()).is_err());
    }

    #[tokio::test]
    #[ignore = "requires explicitly built owned fault-injection workers"]
    async fn native_faults_release_ownership_only_after_os_exit() -> Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-engine-fault-{}", nanoid::nanoid!()));
        std::fs::create_dir(&root)?;
        let _directory = scopeguard::guard(root.clone(), |path| {
            let _ = std::fs::remove_dir(path);
        });
        for mode in ["hang", "malformed", "exit", "panic"] {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../.artifacts/engine-build/windows-x64/fixture-{mode}.exe"));
            let sha256 = Sha256::digest(std::fs::read(&path)?)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            let artifact = WorkerArtifact {
                path,
                sha256,
                build_id: format!("fixture-{mode}"),
            };
            let cancellation = ProbeCancellation::new();
            let cancel = cancellation.cancel_handle();
            let mut client = EngineClient::start(&root, artifact, cancellation).await?;
            let mut request = ProbeRequest {
                transport: Transport::WireGuard,
                ipv6: false,
                targets: vec!["127.0.0.1:9".parse()?],
                identity: Identity::WireGuard(WgIdentity {
                    private_key: STANDARD.encode([7; 32]),
                    peer_public_key: STANDARD.encode([9; 32]),
                    ipv4: "172.16.0.2".parse()?,
                    ipv6: None,
                    reserved: [1, 2, 3],
                }),
                parameters: Parameters { awg: None, sni: None },
                timeout_ms: 30000,
                tunnel_limit: 4,
            };
            let job = client.submit(&request).await?;
            assert!(matches!(client.next_event().await?.payload, EventPayload::Accepted));
            assert!(crate::core::service::reserve_sidecar().await.is_err());
            if mode != "hang" {
                assert!(
                    tokio::time::timeout(Duration::from_secs(2), client.next_event())
                        .await?
                        .is_err()
                );
            } else {
                if let Identity::WireGuard(identity) = &mut request.identity {
                    identity.private_key = "x".repeat(200000);
                }
                assert!(
                    tokio::time::timeout(Duration::from_millis(100), client.submit(&request))
                        .await
                        .is_err()
                );
                let error = tokio::time::timeout(Duration::from_millis(100), client.cancel(&job))
                    .await?
                    .err()
                    .unwrap();
                assert_eq!(
                    error.downcast_ref::<WarpFailure>().unwrap().reason,
                    FailureReason::InvalidOutput
                );
                cancel.send(true)?;
                assert!(client.next_event().await.is_err());
            }
            let started = Instant::now();
            client.shutdown().await?;
            assert!(started.elapsed() < Duration::from_secs(8));
            assert!(!crate::core::warp_children::has_pending());
            let execution = crate::core::service::reserve_sidecar().await?;
            drop(execution);
        }
        assert_eq!(std::fs::read_dir(&root)?.count(), 0);
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires the explicitly built owned Go development worker"]
    async fn native_go_worker_cancels_probe_and_confirms_os_exit() -> Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-engine-{}", nanoid::nanoid!()));
        std::fs::create_dir(&root)?;
        let _directory = scopeguard::guard(root.clone(), |path| {
            let _ = std::fs::remove_dir(path);
        });
        let artifact = || {
            if let Ok(root) = std::env::var("TROPA_ENGINE_FIXTURE_ROOT") {
                WorkerArtifact::packaged(Path::new(&root)).expect("compiled worker binding")
            } else {
                WorkerArtifact {
                    path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../.artifacts/engine-build/windows-x64/tropa-probe-dev.exe"),
                    sha256: "32289496765883e5a46444ac3a22da05cfd79f472ebf5971f9f8a4a2bcab301c".into(),
                    build_id: "development".into(),
                }
            }
        };
        let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await?;
        let mut client = EngineClient::start(&root, artifact(), ProbeCancellation::new()).await?;
        let request = ProbeRequest {
            transport: Transport::WireGuard,
            ipv6: false,
            targets: vec![peer.local_addr()?],
            identity: Identity::WireGuard(WgIdentity {
                private_key: STANDARD.encode([7; 32]),
                peer_public_key: STANDARD.encode([9; 32]),
                ipv4: "172.16.0.2".parse()?,
                ipv6: None,
                reserved: [1, 2, 3],
            }),
            parameters: Parameters { awg: None, sni: None },
            timeout_ms: 30000,
            tunnel_limit: 4,
        };
        let job = client.submit(&request).await?;
        assert!(matches!(client.next_event().await?.payload, EventPayload::Accepted));
        let mut packet = [0; 2048];
        let (size, _) = tokio::time::timeout(Duration::from_secs(3), peer.recv_from(&mut packet)).await??;
        assert_eq!(size, 148);
        assert_eq!(&packet[1..4], &[1, 2, 3]);
        client.cancel(&job).await?;
        let terminal = tokio::time::timeout(Duration::from_secs(3), client.next_event()).await??;
        assert!(matches!(
            terminal.payload,
            EventPayload::JobFinished(Outcome::Cancelled)
        ));
        client.shutdown().await?;
        assert!(!crate::core::warp_children::has_pending());
        assert_eq!(std::fs::read_dir(&root)?.count(), 0);

        let store = crate::config::warp_account::WarpAccountStore::new(root.clone());
        let account =
            crate::config::warp_account::WarpAccount::from_warpscout_json(&serde_json::to_vec(&serde_json::json!({
                "id":"synthetic-search", "token":"synthetic-search-token",
                "private_key":STANDARD.encode([7u8;32]), "peer_public_key":STANDARD.encode([9u8;32]),
                "ipv4":"172.16.0.2", "ipv6":null
            }))?)?;
        store.save(&account).await?;
        let account_path = root.join("secrets/warp-account.json");
        let account_before = std::fs::read(&account_path)?;
        let parent = ProbeCancellation::new();
        let mut client = EngineClient::start(&root, artifact(), ProbeCancellation::new()).await?;
        let cancellation = ProbeCancellation::linked(&parent, Instant::now() + Duration::from_millis(250));
        let policy = crate::config::warp_selection::SelectionPolicy {
            allowed_transports: vec![crate::config::warp_profiles::TransportKind::WireGuard],
            ..Default::default()
        };
        let (output, mut events) = mpsc::channel(16);
        let result = super::super::runner::run_with_client(
            &root,
            &mut client,
            &policy,
            false,
            false,
            false,
            &[],
            Instant::now(),
            17,
            &cancellation,
            &output,
            None,
        )
        .await;
        assert!(result.err().is_some_and(|error| {
            error
                .downcast_ref::<WarpFailure>()
                .is_some_and(|failure| failure.reason == FailureReason::Cancelled)
        }));
        let super::super::runner::SearchEvent::Prepared(baselines) = events.try_recv()? else {
            anyhow::bail!("Missing prepared profiles");
        };
        assert_eq!(baselines.len(), 1);
        assert_eq!(baselines[0].endpoint.to_string(), "188.114.98.58:2408");
        assert!(matches!(
            events.try_recv()?,
            super::super::runner::SearchEvent::Started { .. }
        ));
        assert!(crate::core::warp_children::has_pending());
        client.shutdown().await?;
        assert!(!crate::core::warp_children::has_pending());
        let mut client = EngineClient::start(&root, artifact(), ProbeCancellation::new()).await?;
        let start = Instant::now();
        let mut plan =
            super::super::scheduler::SearchSchedule::new(&policy, false, false, start, 17, client.parameter_sets())?;
        let initial = plan.admit(start).expect("initial batch");
        assert!(plan.finish(initial.token, start));
        let cancellation = ProbeCancellation::linked(&parent, start + Duration::from_millis(250));
        let (output, mut events) = mpsc::channel(16);
        let result = super::super::runner::run_with_client(
            &root,
            &mut client,
            &policy,
            false,
            false,
            false,
            &[],
            start,
            17,
            &cancellation,
            &output,
            Some(plan),
        )
        .await;
        assert!(result.err().is_some_and(|error| {
            error
                .downcast_ref::<WarpFailure>()
                .is_some_and(|failure| failure.reason == FailureReason::Cancelled)
        }));
        let super::super::runner::SearchEvent::Prepared(baselines) = events.try_recv()? else {
            anyhow::bail!("Missing resumed preparation");
        };
        assert!(
            baselines.is_empty(),
            "Resumption must not restart the initial endpoint pass"
        );
        assert!(matches!(
            events.try_recv()?,
            super::super::runner::SearchEvent::Started { .. }
        ));
        client.shutdown().await?;
        assert!(!crate::core::warp_children::has_pending());
        assert_eq!(std::fs::read(&account_path)?, account_before);
        if let Ok(runtime_root) = std::env::var("TROPA_ENGINE_FIXTURE_ROOT") {
            let catalog_path = root.join("warp-profiles.json");
            std::fs::create_dir(&catalog_path)?;
            let mut catalog = crate::config::warp_profiles::WarpCatalog::default();
            let progress = parking_lot::Mutex::new(crate::core::warp_search::SearchProgress::default());
            let result = tokio::time::timeout(
                Duration::from_secs(8),
                super::super::stream::search(
                    &root,
                    Path::new(&runtime_root),
                    &mut catalog,
                    &policy,
                    false,
                    false,
                    false,
                    Instant::now(),
                    17,
                    &ProbeCancellation::new(),
                    &progress,
                    None,
                ),
            )
            .await?;
            let error = result.err().expect("catalog directory must reject persistence");
            assert!(error.downcast_ref::<std::io::Error>().is_some(), "{error:#}");
            assert!(
                error.downcast_ref::<WarpFailure>().is_none(),
                "write failure must not become cancellation"
            );
            assert!(catalog_path.is_dir());
            assert_eq!(std::fs::read_dir(&catalog_path)?.count(), 0);
            assert!(!crate::core::warp_children::has_pending());
            let execution = crate::core::service::reserve_sidecar().await?;
            drop(execution);
            assert_eq!(std::fs::read(&account_path)?, account_before);
            std::fs::remove_dir(catalog_path)?;
        }
        assert_eq!(std::fs::read_dir(&root)?.count(), 1);
        assert_eq!(std::fs::read_dir(root.join("secrets"))?.count(), 1);
        std::fs::remove_file(account_path)?;
        std::fs::remove_dir(root.join("secrets"))?;

        let mut mismatch = artifact();
        mismatch.sha256 = "0".repeat(64);
        assert!(
            EngineClient::start(&root, mismatch, ProbeCancellation::new())
                .await
                .is_err()
        );
        let mut mismatch = artifact();
        mismatch.build_id = "unexpected-build".into();
        assert!(
            EngineClient::start(&root, mismatch, ProbeCancellation::new())
                .await
                .is_err()
        );
        assert!(!crate::core::warp_children::has_pending());
        Ok(())
    }
}

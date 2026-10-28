use super::warp_failure::{FailureReason, FailureStage, WarpFailure};
use super::warp_health::{Health, HealthMonitor, SessionHealth, health_client};
use crate::{
    config::{
        tropa::{RoutingMode, RoutingPreferences, TropaPreferences},
        warp_profiles::TransportKind,
    },
    core::{
        CoreManager,
        handle::Handle,
        warp_discovery::run_profile_discovery,
        warp_probe::ProbeCancellation,
        warp_runtime::{RuntimeAccount, profile_runtime_config},
    },
    utils::{dirs, help::save_yaml_str},
};
use anyhow::{Result, ensure};
use serde::Serialize;
use std::sync::{
    Arc, LazyLock,
    atomic::{AtomicU64, Ordering},
};

pub(crate) static WARP_CONNECTION: LazyLock<WarpConnection> = LazyLock::new(WarpConnection::default);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ConnectionPhase {
    #[default]
    Disconnected,
    Searching,
    Starting,
    Verifying,
    Connected,
    Disconnecting,
    Failed,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectionView {
    routing: Option<RoutingPreferences>,
    phase: ConnectionPhase,
    health: Health,
    active_profile_id: Option<String>,
    error: Option<&'static str>,
    failure: Option<WarpFailure>,
    needs_disconnect: bool,
    search_only: bool,
    progress: Option<super::warp_discovery::SearchProgress>,
}

impl ConnectionView {
    pub(crate) fn diagnostic_snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "phase": self.phase,
            "health": self.health,
            "failure": self.failure,
            "needsDisconnect": self.needs_disconnect,
            "searchOnly": self.search_only,
            "routing": self.routing.as_ref().map(|routing| serde_json::json!({
                "mode": routing.mode,
                "applicationCount": routing.applications.len(),
            })),
            "search": self.progress.as_ref().map(|progress| progress.diagnostic_snapshot()),
        })
    }

    pub(crate) fn needs_disconnect(&self) -> bool {
        self.needs_disconnect
    }
}

#[derive(Default)]
pub(crate) struct WarpConnection {
    operation: tokio::sync::Mutex<()>,
    state: parking_lot::Mutex<ConnectionView>,
    cancellation: parking_lot::Mutex<Option<tokio::sync::watch::Sender<bool>>>,
    cancel_generation: AtomicU64,
    health: Arc<parking_lot::Mutex<SessionHealth>>,
    health_monitor: parking_lot::Mutex<Option<HealthMonitor>>,
}

impl WarpConnection {
    pub(crate) fn view(&self) -> ConnectionView {
        let mut state = self.state.lock();
        if state.phase == ConnectionPhase::Searching && state.search_only {
            state.progress = Some(super::warp_discovery::SEARCH_PROGRESS.lock().clone());
        }
        state.health = if state.phase == ConnectionPhase::Connected {
            self.health.lock().view(std::time::Instant::now())
        } else {
            Health::Unknown
        };
        state.needs_disconnect = CoreManager::global().owns_tropa_runtime() || super::warp_children::has_pending();
        if state.phase == ConnectionPhase::Connected && CoreManager::global().get_running_sidecar_pid().is_none() {
            state.phase = ConnectionPhase::Failed;
            state.health = Health::Unknown;
            self.stop_health();
            state.active_profile_id = None;
            let failure = WarpFailure::new(FailureStage::Runtime, FailureReason::ProcessExited);
            state.error = Some(failure.message());
            state.failure = Some(failure);
        }
        state.clone()
    }

    pub(crate) fn idle_guard(&self) -> Result<tokio::sync::MutexGuard<'_, ()>> {
        self.operation
            .try_lock()
            .map_err(|_| anyhow::anyhow!("WARP operation is in progress"))
    }

    fn stop_health(&self) {
        self.health_monitor.lock().take();
        self.health.lock().stop();
    }

    fn start_health_monitor(&self, client: reqwest::Client, cancellation: &ProbeCancellation) -> Result<()> {
        let mut monitor = self.health_monitor.lock();
        ensure!(
            !cancellation.is_cancelled() && !Handle::global().is_exiting(),
            WarpFailure::new(FailureStage::Validation, FailureReason::Cancelled)
        );
        let generation = self.health.lock().start(std::time::Instant::now());
        *monitor = Some(HealthMonitor::start(client, Arc::clone(&self.health), generation));
        Ok(())
    }

    pub(crate) fn cancel(&self) {
        self.cancel_generation.fetch_add(1, Ordering::AcqRel);
        if let Some(sender) = self.cancellation.lock().as_ref() {
            sender.send_replace(true);
        }
        self.stop_health();
    }

    fn cancel_generation(&self) -> u64 {
        self.cancel_generation.load(Ordering::Acquire)
    }

    fn begin_cancellation(&self, generation: u64) -> ProbeCancellation {
        let cancellation = ProbeCancellation::new();
        let sender = cancellation.cancel_handle();
        *self.cancellation.lock() = Some(sender.clone());
        if self.cancel_generation() != generation {
            sender.send_replace(true);
        }
        cancellation
    }

    fn phase(&self, phase: ConnectionPhase) {
        self.state.lock().phase = phase;
    }

    pub(crate) async fn run(&self, refresh: bool) -> Result<ConnectionView> {
        self.run_scoped(refresh, None, false).await
    }

    pub(crate) async fn search(&self, transport: Option<TransportKind>, advanced: bool) -> Result<ConnectionView> {
        self.run_scoped(true, transport, advanced).await
    }

    async fn run_scoped(
        &self,
        refresh: bool,
        transport: Option<TransportKind>,
        advanced: bool,
    ) -> Result<ConnectionView> {
        let generation = self.cancel_generation();
        let _operation = self.idle_guard()?;
        super::warp_children::wait_for_exit().await?;
        ensure!(!Handle::global().is_exiting(), "Application is exiting");
        let manager = CoreManager::global();
        ensure!(
            manager.get_running_sidecar_pid().is_none() && !manager.owns_tropa_runtime(),
            "Disconnect before searching or connecting"
        );
        if !refresh {
            ensure!(
                crate::core::runstate::RUN_STATE.state().is_admin,
                "Перезапустите Tropa от имени администратора для подключения TUN."
            );
        }
        let cancellation = self.begin_cancellation(generation);
        let _clear_cancellation = scopeguard::guard((), |_| {
            self.cancellation.lock().take();
        });
        *super::warp_discovery::SEARCH_PROGRESS.lock() = Default::default();
        *self.state.lock() = ConnectionView {
            phase: ConnectionPhase::Searching,
            search_only: refresh,
            ..Default::default()
        };
        let result = self.establish(refresh, &cancellation, transport, advanced).await;
        let search_progress = refresh.then(|| super::warp_discovery::SEARCH_PROGRESS.lock().clone());
        self.state.lock().progress = search_progress.clone();
        if let Err(error) = result {
            // Failed shutdown remains owned and blocks a new connection and application exit.
            let core_stopped = if manager.owns_tropa_runtime() {
                manager.stop_core().await.is_ok()
            } else {
                true
            };
            let stopped = core_stopped && super::warp_children::wait_for_exit().await.is_ok();
            let cancelled = cancellation.is_cancelled();
            let failure = if !stopped {
                WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending)
            } else if cancelled {
                WarpFailure::new(FailureStage::Validation, FailureReason::Cancelled)
            } else {
                let stage = match self.state.lock().phase {
                    ConnectionPhase::Searching => FailureStage::Scanner,
                    ConnectionPhase::Starting => FailureStage::Runtime,
                    _ => FailureStage::Validation,
                };
                WarpFailure::from_error(&error, stage, FailureReason::InvalidOutput)
            };
            *self.state.lock() = ConnectionView {
                phase: if cancelled && stopped {
                    ConnectionPhase::Disconnected
                } else {
                    ConnectionPhase::Failed
                },
                active_profile_id: None,
                error: (!(cancelled && stopped)).then(|| failure.message()),
                failure: (!(cancelled && stopped)).then_some(failure),
                needs_disconnect: !stopped,
                search_only: refresh,
                progress: search_progress,
                routing: None,
                health: Health::Unknown,
            };
            if cancelled && stopped {
                return Ok(self.view());
            }
            return Err(failure.into());
        }
        Ok(self.view())
    }

    async fn establish(
        &self,
        refresh: bool,
        cancellation: &ProbeCancellation,
        transport: Option<TransportKind>,
        advanced: bool,
    ) -> Result<()> {
        let root = dirs::app_home_dir()?;
        let resources = dirs::app_resources_dir()?;
        let routing = {
            let _write = crate::config::Config::lock_config_write().await;
            TropaPreferences::load(&root.join("tropa.yaml"))
                .await
                .map_err(|error| WarpFailure::from_error(&error, FailureStage::Routing, FailureReason::InvalidOutput))?
                .routing
        };
        if !refresh {
            routing.validate_connection().await.map_err(|error| {
                WarpFailure::from_error(&error, FailureStage::Routing, FailureReason::InvalidOutput)
            })?;
        }
        let report = run_profile_discovery(
            root.clone(),
            resources.join("verge-mihomo.exe"),
            resources.parent().ok_or_else(|| anyhow::anyhow!("Runtime directory is missing"))?.to_path_buf(),
            refresh,
            cancellation,
            transport,
            advanced,
        )
        .await?;
        ensure!(
            !cancellation.is_cancelled() && !Handle::global().is_exiting(),
            "WARP operation cancelled"
        );
        if refresh {
            self.phase(ConnectionPhase::Disconnected);
            return Ok(());
        }
        let policy = TropaPreferences::load(&root.join("tropa.yaml")).await?.selection_policy;
        ensure!(
            policy.allows_observation(&report.profile.transport, &report.observation, false),
            WarpFailure::new(FailureStage::Validation, FailureReason::PolicyRestricted)
        );
        self.phase(ConnectionPhase::Starting);
        let account = RuntimeAccount::load(&root, &report.profile).await.map_err(|error| {
            WarpFailure::from_error(&error, FailureStage::Registration, FailureReason::InvalidOutput)
                .for_transport(report.profile.transport.kind())
        })?;
        let mut config = profile_runtime_config(account.identity(), &report.profile, &routing).map_err(|error| {
            WarpFailure::from_error(&error, FailureStage::Runtime, FailureReason::Unsupported)
                .for_transport(report.profile.transport.kind())
        })?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let password = nanoid::nanoid!(40);
        config["skip-auth-prefixes"] = serde_json::json!([]);
        config["listeners"] = serde_json::json!([{
            "name":"tropa-verify", "type":"mixed", "listen":"127.0.0.1",
            "port":port, "proxy":"WARP", "udp":false,
            "users":[{"username":"tropa-verify","password":password}]
        }]);
        drop(listener);
        let health_client = health_client(port, &password)?;
        let verification = (routing.mode == RoutingMode::Selected).then_some((port, password));
        let runtime = root.join("secrets/tropa-runtime.yaml");
        save_yaml_str(&runtime, &serde_yaml_ng::to_string(&config)?).await?;
        let manager = CoreManager::global();
        if let Err(error) = manager.start_tropa_core(runtime.clone()).await {
            if !manager.owns_tropa_runtime() {
                let _ = tokio::fs::remove_file(runtime).await;
            }
            return Err(WarpFailure::from_error(&error, FailureStage::Runtime, FailureReason::ProcessExited).into());
        }
        ensure!(
            !cancellation.is_cancelled() && !Handle::global().is_exiting(),
            "WARP operation cancelled"
        );
        manager
            .set_tropa_tun(true)
            .await
            .map_err(|error| WarpFailure::from_error(&error, FailureStage::Routing, FailureReason::InvalidOutput))?;
        self.phase(ConnectionPhase::Verifying);
        tokio::select! {
            result = verify_system_warp(verification, &policy) => result.map_err(|error| {
                WarpFailure::from_error(&error, FailureStage::Validation, FailureReason::Unreachable)
                    .for_transport(report.profile.transport.kind())
            })?,
            _ = cancellation.cancelled() => anyhow::bail!("WARP operation cancelled"),
        }
        ensure!(
            !cancellation.is_cancelled() && !Handle::global().is_exiting(),
            "WARP operation cancelled"
        );
        *self.state.lock() = ConnectionView {
            routing: Some(routing),
            phase: ConnectionPhase::Connected,
            health: Health::Healthy,
            active_profile_id: Some(report.profile.id),
            error: None,
            failure: None,
            needs_disconnect: true,
            search_only: false,
            progress: None,
        };
        self.stop_health();
        self.start_health_monitor(health_client, cancellation)?;
        Ok(())
    }

    pub(crate) async fn disconnect(&self) -> Result<ConnectionView> {
        self.cancel();
        let _operation = self.operation.lock().await;
        self.phase(ConnectionPhase::Disconnecting);
        if let Err(error) = super::warp_children::wait_for_exit().await {
            let mut state = self.state.lock();
            state.phase = ConnectionPhase::Failed;
            let failure = WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending);
            state.error = Some(failure.message());
            state.failure = Some(failure);
            let _ = error;
            return Err(failure.into());
        }
        let manager = CoreManager::global();
        if manager.owns_tropa_runtime()
            && let Err(error) = manager.stop_core().await
        {
            let mut state = self.state.lock();
            state.phase = ConnectionPhase::Failed;
            let failure = WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending);
            state.error = Some(failure.message());
            state.failure = Some(failure);
            let _ = error;
            return Err(failure.into());
        }
        let mut state = self.state.lock();
        let progress = state.search_only.then(|| state.progress.clone()).flatten();
        *state = ConnectionView {
            search_only: progress.is_some(),
            progress,
            ..Default::default()
        };
        drop(state);
        Ok(self.view())
    }
}

async fn verify_system_warp(
    verification: Option<(u16, String)>,
    policy: &crate::config::warp_selection::SelectionPolicy,
) -> Result<()> {
    let mut client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(8));
    if let Some((port, password)) = verification {
        client = client
            .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))?.basic_auth("tropa-verify", &password));
    }
    let client = client.build()?;
    verify_trace(&client, "https://1.1.1.1/cdn-cgi/trace").await?;
    if policy.has_geography() {
        let metadata = super::warp_metadata::fetch_metadata(&client)
            .await
            .map_err(|_| WarpFailure::new(FailureStage::Validation, FailureReason::PolicyRestricted))?;
        ensure!(
            policy.matches_metadata(metadata.node.as_deref(), metadata.country.as_deref(), false),
            WarpFailure::new(FailureStage::Validation, FailureReason::PolicyRestricted)
        );
    }
    Ok(())
}

fn transient_trace_failure(error: &anyhow::Error) -> bool {
    use std::error::Error;
    let Some(request) = error.downcast_ref::<reqwest::Error>() else {
        return false;
    };
    if request.is_timeout() {
        return true;
    }
    if !request.is_connect() {
        return false;
    }
    let mut source = request.source();
    while let Some(cause) = source {
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            return matches!(
                io.kind(),
                std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::TimedOut
            );
        }
        source = cause.source();
    }
    false
}

async fn verify_trace(client: &reqwest::Client, url: &str) -> Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        for attempt in 0..3 {
            let result = read_trace(client, url).await;
            match result {
                Ok(()) => return Ok(()),
                Err(error) if attempt < 2 && transient_trace_failure(&error) => {
                    tokio::time::sleep(std::time::Duration::from_secs(attempt + 1)).await;
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("three attempts always return a result")
    })
    .await
    .map_err(|_| WarpFailure::new(FailureStage::Validation, FailureReason::Timeout))?
}

pub(super) async fn read_trace(client: &reqwest::Client, url: &str) -> Result<()> {
    let mut response = client.get(url).send().await?.error_for_status()?;
    ensure!(
        response.status() == reqwest::StatusCode::OK,
        WarpFailure::new(FailureStage::Validation, FailureReason::InvalidOutput)
    );
    let mut trace = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            trace.len() + chunk.len() <= 16 * 1024,
            WarpFailure::new(FailureStage::Validation, FailureReason::InvalidOutput)
        );
        trace.extend_from_slice(&chunk);
    }
    ensure!(
        std::str::from_utf8(&trace)
            .unwrap_or_default()
            .lines()
            .any(|line| matches!(line.trim(), "warp=on" | "warp=plus")),
        WarpFailure::new(FailureStage::Validation, FailureReason::InvalidOutput)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_omit_private_paths_profile_ids_and_raw_error_text() {
        let state = ConnectionView {
            phase: ConnectionPhase::Failed,
            routing: Some(RoutingPreferences {
                mode: RoutingMode::Selected,
                applications: vec!["C:/private-application-marker/game.exe".into()],
            }),
            active_profile_id: Some("private-profile-marker".into()),
            error: Some("private-error-marker"),
            failure: Some(WarpFailure::new(FailureStage::Runtime, FailureReason::ProcessExited)),
            needs_disconnect: true,
            ..Default::default()
        };
        let report = state.diagnostic_snapshot();
        assert!(!serde_json::to_string(&report).unwrap().contains("private-"));
        assert_eq!(report["phase"], "failed");
        assert_eq!(report["failure"]["reason"], "processExited");
        assert_eq!(report["routing"]["mode"], "selected");
        assert_eq!(report["routing"]["applicationCount"], 1);
        assert_eq!(report["needsDisconnect"], true);
    }

    #[tokio::test]
    async fn transient_trace_timeout_can_recover_before_connection_is_declared_failed() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/trace", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for attempt in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0u8; 2048];
                stream.read(&mut request).await.unwrap();
                if attempt == 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                }
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nwarp=on\n")
                    .await;
            }
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .retry(reqwest::retry::never())
            .timeout(std::time::Duration::from_millis(100))
            .build()
            .unwrap();
        let result = verify_trace(&client, &url).await;
        server.abort();
        assert!(result.is_ok(), "temporary timeout must be retried: {result:?}");
    }

    #[tokio::test]
    async fn trace_authentication_failure_and_no_warp_are_not_retried() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for response in [
            "HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nwarp=off\n",
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/trace", listener.local_addr().unwrap());
            let requests = Arc::new(AtomicUsize::new(0));
            let counter = Arc::clone(&requests);
            let server = tokio::spawn(async move {
                loop {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let mut request = [0u8; 2048];
                    stream.read(&mut request).await.unwrap();
                    counter.fetch_add(1, Ordering::Relaxed);
                    let _ = stream.write_all(response.as_bytes()).await;
                }
            });
            let client = reqwest::Client::builder()
                .no_proxy()
                .retry(reqwest::retry::never())
                .build()
                .unwrap();
            assert!(verify_trace(&client, &url).await.is_err());
            assert_eq!(requests.load(Ordering::Relaxed), 1);
            server.abort();
        }
    }

    #[test]
    fn completed_search_keeps_its_progress_in_later_snapshots() {
        let connection = WarpConnection::default();
        *connection.state.lock() = ConnectionView {
            search_only: true,
            progress: Some(Default::default()),
            ..Default::default()
        };
        assert!(connection.view().progress.is_some());
        assert!(connection.view().progress.is_some());
    }

    #[tokio::test]
    async fn cancelling_search_keeps_its_final_counts_after_cleanup() {
        let connection = WarpConnection::default();
        *connection.state.lock() = ConnectionView {
            search_only: true,
            progress: Some(Default::default()),
            ..Default::default()
        };
        assert!(connection.disconnect().await.unwrap().progress.is_some());
    }

    #[tokio::test]
    async fn cancellation_before_token_publication_cannot_start_a_tunnel() {
        let connection = WarpConnection::default();
        let generation = connection.cancel_generation();
        connection.cancel();
        let cancellation = connection.begin_cancellation(generation);
        assert!(cancellation.is_cancelled());
        assert!(
            connection
                .start_health_monitor(reqwest::Client::new(), &cancellation)
                .is_err()
        );
        assert!(connection.health_monitor.lock().is_none());
        assert_eq!(
            connection.health.lock().view(std::time::Instant::now()),
            Health::Unknown
        );
    }

    #[tokio::test]
    async fn cancellation_reaches_an_already_published_operation() {
        let connection = WarpConnection::default();
        let cancellation = connection.begin_cancellation(connection.cancel_generation());
        connection.cancel();
        assert!(cancellation.is_cancelled());
    }

    #[tokio::test]
    async fn selection_is_blocked_until_the_search_releases_ownership() {
        let connection = WarpConnection::default();
        let search = connection.idle_guard().unwrap();
        assert!(connection.idle_guard().is_err());
        drop(search);
        assert!(connection.idle_guard().is_ok());
    }
}

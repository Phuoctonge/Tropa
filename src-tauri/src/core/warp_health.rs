use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Health {
    Healthy,
    Degraded,
    Unreachable,
    #[default]
    Unknown,
}

#[derive(Default)]
pub(crate) struct SessionHealth {
    generation: u64,
    active: bool,
    health: Health,
    failures: u8,
    checked: Option<Instant>,
}

impl SessionHealth {
    pub(crate) fn start(&mut self, now: Instant) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.active = true;
        self.health = Health::Healthy;
        self.failures = 0;
        self.checked = Some(now);
        self.generation
    }

    pub(crate) fn stop(&mut self) {
        self.active = false;
        self.health = Health::Unknown;
        self.checked = None;
    }

    fn active(&self, generation: u64) -> bool {
        self.active && self.generation == generation
    }

    fn observe(&mut self, generation: u64, success: bool, now: Instant) -> bool {
        if !self.active(generation) {
            return false;
        }
        self.checked = Some(now);
        self.failures = if success { 0 } else { self.failures.saturating_add(1) };
        self.health = match self.failures {
            0 => Health::Healthy,
            1 | 2 => Health::Degraded,
            _ => Health::Unreachable,
        };
        true
    }

    pub(crate) fn view(&self, now: Instant) -> Health {
        if !self.active
            || self
                .checked
                .is_none_or(|checked| now.saturating_duration_since(checked) > Duration::from_secs(45))
        {
            Health::Unknown
        } else {
            self.health
        }
    }
}

pub(crate) fn health_client(port: u16, password: &str) -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .no_proxy()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))?.basic_auth("tropa-verify", password))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build()?)
}

pub(crate) struct HealthMonitor {
    task: tokio::task::JoinHandle<()>,
}

impl HealthMonitor {
    pub(crate) fn start(
        client: reqwest::Client,
        state: Arc<parking_lot::Mutex<SessionHealth>>,
        generation: u64,
    ) -> Self {
        Self::spawn(
            client,
            state,
            generation,
            "https://1.1.1.1/cdn-cgi/trace".into(),
            Duration::from_secs(15),
        )
    }

    fn spawn(
        client: reqwest::Client,
        state: Arc<parking_lot::Mutex<SessionHealth>>,
        generation: u64,
        url: String,
        interval: Duration,
    ) -> Self {
        let task = tokio::spawn(async move {
            let mut ticks = tokio::time::interval_at(tokio::time::Instant::now() + interval, interval);
            ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticks.tick().await;
                if !state.lock().active(generation) {
                    break;
                }
                let success = matches!(
                    tokio::time::timeout(
                        Duration::from_secs(5),
                        super::warp_connection::read_trace(&client, &url)
                    )
                    .await,
                    Ok(Ok(()))
                );
                if !state.lock().observe(generation, success, Instant::now()) {
                    break;
                }
            }
        });
        Self { task }
    }
}

impl Drop for HealthMonitor {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_core_does_not_hide_failed_data_path_and_stale_sessions_cannot_change_health() {
        let now = std::time::Instant::now();
        let mut state = SessionHealth::default();
        let first = state.start(now);
        assert_eq!(state.view(now), Health::Healthy);
        assert!(state.observe(first, false, now + Duration::from_secs(1)));
        assert_eq!(state.view(now + Duration::from_secs(1)), Health::Degraded);
        state.observe(first, false, now + Duration::from_secs(2));
        state.observe(first, false, now + Duration::from_secs(3));
        assert_eq!(state.view(now + Duration::from_secs(3)), Health::Unreachable);
        state.observe(first, true, now + Duration::from_secs(4));
        assert_eq!(state.view(now + Duration::from_secs(4)), Health::Healthy);
        assert_eq!(state.view(now + Duration::from_secs(60)), Health::Unknown);
        let second = state.start(now + Duration::from_secs(61));
        assert!(!state.observe(first, false, now + Duration::from_secs(62)));
        assert_eq!(state.view(now + Duration::from_secs(62)), Health::Healthy);
        state.stop();
        assert!(!state.observe(second, true, now + Duration::from_secs(63)));
        assert_eq!(state.view(now + Duration::from_secs(63)), Health::Unknown);
    }

    #[tokio::test]
    async fn monitor_uses_authenticated_pinned_proxy_and_recovers_without_replacing_session() -> anyhow::Result<()> {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let recovered = Arc::new(AtomicBool::new(false));
        let response_state = Arc::clone(&recovered);
        let server = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut bytes = [0u8; 1024];
                    let count = stream.read(&mut bytes).await.unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&bytes[..count]);
                }
                let request = String::from_utf8(request).unwrap().to_ascii_lowercase();
                assert!(request.starts_with("get http://tropa-health.invalid/trace "));
                assert!(request.contains("proxy-authorization: basic dhjvcgetdmvyawz5onrva2vu"));
                let body = if response_state.load(Ordering::Acquire) {
                    "warp=on\n"
                } else {
                    "warp=off\n"
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let state = Arc::new(parking_lot::Mutex::new(SessionHealth::default()));
        let generation = state.lock().start(std::time::Instant::now());
        let monitor = HealthMonitor::spawn(
            health_client(port, "token")?,
            Arc::clone(&state),
            generation,
            "http://tropa-health.invalid/trace".into(),
            Duration::from_millis(5),
        );
        for expected in [Health::Unreachable, Health::Healthy] {
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if state.lock().view(std::time::Instant::now()) == expected {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            })
            .await?;
            recovered.store(true, Ordering::Release);
        }
        let task = monitor.task.abort_handle();
        state.lock().stop();
        drop(monitor);
        tokio::time::timeout(Duration::from_secs(1), async {
            while !task.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert_eq!(state.lock().view(std::time::Instant::now()), Health::Unknown);
        server.abort();
        let _ = server.await;
        Ok(())
    }
}

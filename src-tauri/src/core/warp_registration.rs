use super::warp_failure::{FailureReason, FailureStage, WarpFailure};
use crate::config::warp_account::{WarpAccount, WarpAccountStatus, WarpAccountStore};
use anyhow::{Result, anyhow, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::time::Duration;

#[async_trait::async_trait]
pub(crate) trait WarpRegistrar: Send + Sync {
    async fn create(&self) -> Result<WarpAccount>;
    async fn enable(&self, account: &WarpAccount) -> Result<()>;
}

pub(crate) struct HttpWarpRegistrar {
    client: reqwest::Client,
    endpoint: reqwest::Url,
}

#[derive(Debug)]
struct UnusableWarpRegistration;

impl std::fmt::Display for UnusableWarpRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("WARP registration is no longer usable")
    }
}

impl std::error::Error for UnusableWarpRegistration {}

impl HttpWarpRegistrar {
    pub(crate) fn new() -> Result<Self> {
        Self::at_endpoint("https://api.devices.cloudflare.com/v0a4005/reg")
    }

    fn at_endpoint(endpoint: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .user_agent("okhttp/3.12.1")
            .build()
            .map_err(|_| anyhow!("Unable to initialize WARP registration client"))?;
        Ok(Self {
            client,
            endpoint: reqwest::Url::parse(endpoint).map_err(|_| anyhow!("Invalid WARP registration endpoint"))?,
        })
    }

    async fn request(
        &self,
        method: reqwest::Method,
        url: reqwest::Url,
        token: Option<&str>,
        body: serde_json::Value,
    ) -> Result<Vec<u8>> {
        let activation = method == reqwest::Method::PATCH;
        let mut request = self
            .client
            .request(method, url)
            .header("CF-Client-Version", "a-6.11-2223")
            .json(&body);
        if let Some(token) = token {
            request = request.bearer_auth(token)
        }
        let mut response = request.send().await.map_err(|error| {
            WarpFailure::from_error(&error.into(), FailureStage::Registration, FailureReason::Unreachable)
        })?;
        if activation && matches!(response.status().as_u16(), 401 | 404 | 410) {
            return Err(UnusableWarpRegistration.into());
        }
        ensure!(response.status().is_success(), "WARP registration API rejected request");
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| {
            WarpFailure::from_error(&error.into(), FailureStage::Registration, FailureReason::InvalidOutput)
        })? {
            ensure!(
                bytes.len() + chunk.len() <= 64 * 1024,
                "WARP registration response is too large"
            );
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

#[async_trait::async_trait]
impl WarpRegistrar for HttpWarpRegistrar {
    async fn create(&self) -> Result<WarpAccount> {
        let mut secret = [0u8; 32];
        getrandom::fill(&mut secret).map_err(|_| anyhow!("Unable to generate WARP registration key"))?;
        let public = x25519_dalek::x25519(secret, x25519_dalek::X25519_BASEPOINT_BYTES);
        let response = self
            .request(
                reqwest::Method::POST,
                self.endpoint.clone(),
                None,
                serde_json::json!({"key":STANDARD.encode(public)}),
            )
            .await?;
        WarpAccount::from_api_response(&response, STANDARD.encode(secret))
    }

    async fn enable(&self, account: &WarpAccount) -> Result<()> {
        let mut url = self.endpoint.clone();
        url.path_segments_mut()
            .map_err(|_| anyhow!("Invalid WARP registration endpoint"))?
            .push(account.id());
        self.request(
            reqwest::Method::PATCH,
            url,
            Some(account.token()),
            serde_json::json!({"warp_enabled":true}),
        )
        .await?;
        Ok(())
    }
}

pub(crate) async fn ensure_registration(
    store: &WarpAccountStore,
    registrar: &impl WarpRegistrar,
) -> Result<WarpAccountStatus> {
    if let Some(account) = store.load().await? {
        return Ok(account.status());
    }
    create_registration(store, registrar, None).await
}

pub(crate) async fn reset_registration(
    store: &WarpAccountStore,
    registrar: &impl WarpRegistrar,
) -> Result<WarpAccountStatus> {
    let current = store.load().await.ok().flatten();
    if store.load_pending().await.is_err() {
        store.clear_pending().await?;
    }
    create_registration(store, registrar, current.as_ref()).await
}

async fn create_registration(
    store: &WarpAccountStore,
    registrar: &impl WarpRegistrar,
    current: Option<&WarpAccount>,
) -> Result<WarpAccountStatus> {
    let mut pending = store.load_pending().await?;
    if pending
        .as_ref()
        .zip(current)
        .is_some_and(|(pending, current)| pending.id() == current.id())
    {
        store.clear_pending().await?;
        pending = None;
    }
    let account = match pending {
        Some(account) => account,
        None => {
            let account = registrar.create().await?;
            store.save_pending(&account).await?;
            account
        }
    };
    if let Err(error) = registrar.enable(&account).await {
        if error.is::<UnusableWarpRegistration>() {
            store.clear_pending().await?;
        }
        return Err(error);
    }
    store.save(&account).await?;
    // A committed account stays authoritative even if pending cleanup fails.
    let _ = store.clear_pending().await;
    Ok(account.status())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::warp_account::{WarpAccount, WarpAccountStore};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn http_server(responses: Vec<(u16, String)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}/reg", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0u8; 1024];
                    let count = stream.read(&mut chunk).unwrap();
                    assert_ne!(count, 0);
                    bytes.extend_from_slice(&chunk[..count]);
                    if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length: usize = headers
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|value| value.trim().parse().unwrap())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            requests
        });
        (address, handle)
    }

    #[tokio::test]
    async fn direct_api_sends_only_public_key_and_handles_errors_safely() -> anyhow::Result<()> {
        let response = serde_json::json!({
            "id":"synthetic-device", "token":"synthetic-token",
            "config":{"peers":[{"public_key":STANDARD.encode([9u8;32])}],
                "interface":{"addresses":{"v4":"172.16.0.2","v6":""}}}
        })
        .to_string();
        let (endpoint, server) = http_server(vec![(200, response.clone()), (200, "{}".into())]);
        let registrar = HttpWarpRegistrar::at_endpoint(&endpoint)?;
        let account = registrar.create().await?;
        registrar.enable(&account).await?;
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("POST /reg "));
        assert!(requests[1].starts_with("PATCH /reg/synthetic-device "));
        let public: serde_json::Value = serde_json::from_str(requests[0].split("\r\n\r\n").nth(1).unwrap())?;
        assert_eq!(public.as_object().unwrap().len(), 1);
        assert!(
            requests[1]
                .to_lowercase()
                .contains("authorization: bearer synthetic-token")
        );
        assert!(!requests[0].contains("synthetic-token"));
        assert!(requests[1].contains("\"warp_enabled\":true"));
        let private = STANDARD.decode(account.test_private_key())?;
        let secret: [u8; 32] = private.try_into().unwrap();
        assert_eq!(
            public["key"],
            STANDARD.encode(x25519_dalek::x25519(secret, x25519_dalek::X25519_BASEPOINT_BYTES))
        );
        for (code, body) in [
            (429, "secret-response-marker".into()),
            (200, "secret-response-marker".into()),
            (200, "x".repeat(64 * 1024 + 1)),
        ] {
            let (endpoint, server) = http_server(vec![(code, body)]);
            let error = HttpWarpRegistrar::at_endpoint(&endpoint)?
                .create()
                .await
                .err()
                .expect("bad API response accepted");
            assert!(!format!("{error:#}").contains("secret-response-marker"));
            server.join().unwrap();
        }
        let root = std::env::temp_dir().join(format!("tropa-rejected-registration-{}", nanoid::nanoid!()));
        tokio::fs::create_dir(&root).await?;
        let store = WarpAccountStore::new(root.clone());
        let (endpoint, server) = http_server(vec![(200, response), (404, "secret-response-marker".into())]);
        assert!(
            ensure_registration(&store, &HttpWarpRegistrar::at_endpoint(&endpoint)?)
                .await
                .is_err()
        );
        server.join().unwrap();
        assert!(!tokio::fs::try_exists(root.join("secrets/warp-pending.json")).await?);
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }

    struct FakeRegistrar {
        created: AtomicUsize,
        fail_enable: AtomicBool,
    }

    #[async_trait::async_trait]
    impl WarpRegistrar for FakeRegistrar {
        async fn create(&self) -> anyhow::Result<WarpAccount> {
            let number = self.created.fetch_add(1, Ordering::SeqCst);
            WarpAccount::from_warpscout_json(&serde_json::to_vec(&serde_json::json!({
                "id": format!("synthetic-device-{number}"), "token": "synthetic-token",
                "private_key": STANDARD.encode([7_u8;32]), "peer_public_key": STANDARD.encode([9_u8;32]),
                "ipv4": "172.16.0.2", "ipv6": null
            }))?)
        }

        async fn enable(&self, _: &WarpAccount) -> anyhow::Result<()> {
            anyhow::ensure!(!self.fail_enable.load(Ordering::SeqCst), "Synthetic enable failure");
            Ok(())
        }
    }

    #[tokio::test]
    async fn reuse_and_reset_preserve_committed_account_and_resume_pending() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-registration-test-{}", nanoid::nanoid!()));
        tokio::fs::create_dir(&root).await?;
        let store = WarpAccountStore::new(root.clone());
        let registrar = FakeRegistrar {
            created: AtomicUsize::new(0),
            fail_enable: AtomicBool::new(false),
        };
        ensure_registration(&store, &registrar).await?;
        ensure_registration(&store, &registrar).await?;
        assert_eq!(registrar.created.load(Ordering::SeqCst), 1);
        let path = root.join("secrets/warp-account.json");
        let before = tokio::fs::read(&path).await?;
        registrar.fail_enable.store(true, Ordering::SeqCst);
        assert!(reset_registration(&store, &registrar).await.is_err());
        assert_eq!(tokio::fs::read(&path).await?, before);
        assert!(tokio::fs::try_exists(root.join("secrets/warp-pending.json")).await?);
        ensure_registration(&store, &registrar).await?;
        assert_eq!(registrar.created.load(Ordering::SeqCst), 2);
        registrar.fail_enable.store(false, Ordering::SeqCst);
        reset_registration(&store, &registrar).await?;
        assert_eq!(registrar.created.load(Ordering::SeqCst), 2);
        assert_ne!(tokio::fs::read(&path).await?, before);
        let committed = store.load().await?.unwrap();
        store.save_pending(&committed).await?;
        reset_registration(&store, &registrar).await?;
        assert_eq!(registrar.created.load(Ordering::SeqCst), 3);
        tokio::fs::write(&path, b"corrupt").await?;
        assert!(ensure_registration(&store, &registrar).await.is_err());
        assert_eq!(registrar.created.load(Ordering::SeqCst), 3);
        reset_registration(&store, &registrar).await?;
        assert_eq!(registrar.created.load(Ordering::SeqCst), 4);
        let pending_path = root.join("secrets/warp-pending.json");
        tokio::fs::write(&pending_path, b"corrupt-pending").await?;
        ensure_registration(&store, &registrar).await?;
        assert_eq!(registrar.created.load(Ordering::SeqCst), 4);
        reset_registration(&store, &registrar).await?;
        assert_eq!(registrar.created.load(Ordering::SeqCst), 5);
        tokio::fs::remove_file(&path).await?;
        tokio::fs::write(&pending_path, b"corrupt-pending").await?;
        assert!(store.status().await.is_err());
        reset_registration(&store, &registrar).await?;
        assert_eq!(registrar.created.load(Ordering::SeqCst), 6);
        tokio::fs::remove_file(path).await?;
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }
}

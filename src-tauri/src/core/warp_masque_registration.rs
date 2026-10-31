#[cfg(test)]
use super::warp_failure::{FailureReason, WarpFailure};
use crate::config::warp_masque_account::{MasqueAccount, MasqueAccountStore, MasqueKeys, PendingMasque};
use anyhow::{Result, anyhow, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::time::Duration;

pub(crate) struct HttpMasqueRegistrar {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    route: std::sync::Arc<super::warp_registration_http::RegistrationRoute>,
}

#[derive(Debug)]
pub(crate) struct UnusableMasqueRegistration;
impl std::fmt::Display for UnusableMasqueRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("MASQUE registration is no longer usable; reset is required")
    }
}
impl std::error::Error for UnusableMasqueRegistration {}

impl HttpMasqueRegistrar {
    pub(crate) fn using_route(
        mut self,
        route: std::sync::Arc<super::warp_registration_http::RegistrationRoute>,
    ) -> Self {
        self.route = route;
        self
    }

    pub(crate) fn new() -> Result<Self> {
        Self::at_endpoint("https://api.devices.cloudflare.com/v0a4471/reg")
    }

    fn at_endpoint(endpoint: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .user_agent("WARP for Android")
            .build()
            .map_err(|_| anyhow!("Unable to initialize MASQUE registration client"))?;
        Ok(Self {
            client,
            route: Default::default(),
            endpoint: reqwest::Url::parse(endpoint).map_err(|_| anyhow!("Invalid MASQUE registration endpoint"))?,
        })
    }

    async fn request(
        &self,
        method: reqwest::Method,
        url: reqwest::Url,
        token: Option<&str>,
        body: serde_json::Value,
    ) -> Result<Vec<u8>> {
        let enrollment = method == reqwest::Method::PATCH;
        let mut request = self
            .client
            .request(method, url)
            .header("CF-Client-Version", "a-6.35-4471")
            .json(&body);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = self.route.send(&self.client, request.build()?).await?;
        if enrollment && matches!(response.status, 401 | 404 | 410) {
            return Err(UnusableMasqueRegistration.into());
        }
        ensure!(
            (200..300).contains(&response.status),
            "MASQUE registration API rejected request"
        );
        Ok(response.body)
    }

    async fn create(&self) -> Result<PendingMasque> {
        let keys = MasqueKeys::generate()?;
        let mut placeholder = [0u8; 32];
        let mut serial = [0u8; 8];
        getrandom::fill(&mut placeholder).map_err(|_| anyhow!("Unable to generate MASQUE registration placeholder"))?;
        getrandom::fill(&mut serial).map_err(|_| anyhow!("Unable to generate MASQUE device serial"))?;
        let response = self
            .request(
                reqwest::Method::POST,
                self.endpoint.clone(),
                None,
                serde_json::json!({
                    "key": STANDARD.encode(placeholder), "install_id":"", "fcm_token":"",
                    "tos": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    "model":"PC", "serial_number":STANDARD.encode(serial), "os_version":"10",
                    "key_type":"curve25519", "tunnel_type":"wireguard", "locale":"en_US"
                }),
            )
            .await?;
        PendingMasque::from_created(keys, &response)
    }

    async fn enroll(&self, pending: &PendingMasque) -> Result<Vec<u8>> {
        let mut url = self.endpoint.clone();
        url.path_segments_mut()
            .map_err(|_| anyhow!("Invalid MASQUE registration endpoint"))?
            .push(pending.id());
        self.request(reqwest::Method::PATCH, url, Some(pending.token()),
            serde_json::json!({ "key":pending.public_key(), "key_type":"secp256r1", "tunnel_type":"masque", "name":"Tropa" })).await
    }
}

pub(crate) async fn ensure_registration(
    store: &MasqueAccountStore,
    registrar: &HttpMasqueRegistrar,
) -> Result<MasqueAccount> {
    if let Some(account) = store.load().await? {
        return Ok(account);
    }
    complete_registration(store, registrar).await
}

pub(crate) async fn reset_registration(
    store: &MasqueAccountStore,
    registrar: &HttpMasqueRegistrar,
) -> Result<MasqueAccount> {
    let current = store.load().await.ok().flatten();
    match store.load_pending().await {
        Ok(Some(pending)) if current.as_ref().is_some_and(|account| account.id() == pending.id()) => {
            store.clear_pending().await?;
        }
        Err(_) => store.clear_pending().await?,
        _ => {}
    }
    match complete_registration(store, registrar).await {
        Err(error) if error.is::<UnusableMasqueRegistration>() => {
            store.clear_pending().await?;
            complete_registration(store, registrar).await
        }
        result => result,
    }
}

async fn complete_registration(store: &MasqueAccountStore, registrar: &HttpMasqueRegistrar) -> Result<MasqueAccount> {
    let pending = match store.load_pending().await? {
        Some(pending) => pending,
        None => {
            let pending = registrar.create().await?;
            store.save_pending(&pending).await?;
            pending
        }
    };
    let response = registrar.enroll(&pending).await?;
    let account = MasqueAccount::from_enrollment(pending, &response)?;
    store.save(&account).await?;
    // A committed account stays authoritative even if pending cleanup fails.
    let _ = store.clear_pending().await;
    Ok(account)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::warp_masque_account::{MasqueAccountStore, MasqueKeys};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn server(responses: Vec<Option<(u16, String)>>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        server_before_reply(responses, || {}).await
    }

    async fn server_before_reply(
        responses: Vec<Option<(u16, String)>>,
        before_reply: impl Fn() + Send + 'static,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/reg", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
                let request = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    let mut bytes = Vec::new();
                    loop {
                        let mut chunk = [0u8; 1024];
                        let n = stream.read(&mut chunk).await.unwrap();
                        assert_ne!(n, 0);
                        bytes.extend_from_slice(&chunk[..n]);
                        assert!(bytes.len() <= 64 * 1024);
                        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            let headers = String::from_utf8_lossy(&bytes[..end]);
                            let length: usize = headers
                                .lines()
                                .find_map(|line| {
                                    line.to_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(|v| v.trim().parse().unwrap())
                                })
                                .unwrap_or(0);
                            if bytes.len() >= end + 4 + length {
                                break;
                            }
                        }
                    }
                    String::from_utf8(bytes).unwrap()
                })
                .await
                .unwrap();
                requests.push(request);
                if let Some((status, body)) = response {
                    before_reply();
                    stream.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
                }
            }
            requests
        });
        (endpoint, task)
    }

    fn enrollment_response() -> anyhow::Result<String> {
        let key = serde_json::to_value(MasqueKeys::generate()?)?["public_key"]
            .as_str()
            .unwrap()
            .to_owned();
        Ok(serde_json::json!({"config": {
            "peers":[{"public_key":format!("-----BEGIN PUBLIC KEY-----\n{key}\n-----END PUBLIC KEY-----")}],
            "interface":{"addresses":{"v4":"172.16.0.2","v6":""}}
        }})
        .to_string())
    }

    #[tokio::test]
    async fn failed_reset_preserves_completed_identity_and_next_reset_resumes_pending() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-masque-reset-{}", nanoid::nanoid!()));
        let store = MasqueAccountStore::new(root.clone());
        assert_eq!(
            serde_json::to_value(store.status().await?)?,
            serde_json::json!({"status":"missing"})
        );
        let original =
            PendingMasque::from_created(MasqueKeys::generate()?, br#"{"id":"old-device","token":"old-token"}"#)?;
        let original = MasqueAccount::from_enrollment(original, enrollment_response()?.as_bytes())?;
        store.save(&original).await?;
        let original_bytes = tokio::fs::read(root.join("secrets/masque-account.json")).await?;
        let wg = root.join("secrets/warp-account.json");
        tokio::fs::write(&wg, b"untouched-wg-marker").await?;
        let (endpoint, requests) = server(vec![
            Some((200, r#"{"id":"new-device","token":"new-token"}"#.into())),
            None,
        ])
        .await;
        assert!(
            reset_registration(&store, &HttpMasqueRegistrar::at_endpoint(&endpoint)?)
                .await
                .is_err()
        );
        let requests = requests.await?;
        assert!(requests[0].starts_with("POST /reg "));
        assert!(requests[1].starts_with("PATCH /reg/new-device "));
        assert_eq!(
            tokio::fs::read(root.join("secrets/masque-account.json")).await?,
            original_bytes
        );
        let pending = serde_json::to_value(store.load_pending().await?.unwrap())?;
        assert_ne!(
            pending["keys"],
            serde_json::to_value(&original)?["registration"]["keys"]
        );
        let status = serde_json::to_value(store.status().await?)?;
        assert_eq!(status, serde_json::json!({"status":"registered","hasIpv6":false}));
        let (endpoint, requests) = server(vec![Some((200, enrollment_response()?))]).await;
        let replaced = reset_registration(&store, &HttpMasqueRegistrar::at_endpoint(&endpoint)?).await?;
        let requests = requests.await?;
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("PATCH /reg/new-device "));
        assert_eq!(serde_json::to_value(replaced)?["registration"], pending);
        assert!(store.load_pending().await?.is_none());
        assert_eq!(tokio::fs::read(&wg).await?, b"untouched-wg-marker");
        for name in ["warp-account.json", "masque-account.json"] {
            tokio::fs::remove_file(root.join("secrets").join(name)).await?;
        }
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }

    #[tokio::test]
    async fn explicit_reset_replaces_revoked_pending_but_ensure_preserves_it() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-masque-revoked-{}", nanoid::nanoid!()));
        let store = MasqueAccountStore::new(root.clone());
        let pending = PendingMasque::from_created(
            MasqueKeys::generate()?,
            br#"{"id":"revoked-device","token":"revoked-token"}"#,
        )?;
        store.save_pending(&pending).await?;
        assert_eq!(
            serde_json::to_value(store.status().await?)?,
            serde_json::json!({"status":"pending"})
        );
        let (endpoint, requests) = server(vec![
            Some((401, "synthetic-body".into())),
            Some((200, r#"{"id":"replacement-device","token":"replacement-token"}"#.into())),
            Some((200, enrollment_response()?)),
        ])
        .await;
        let account = reset_registration(&store, &HttpMasqueRegistrar::at_endpoint(&endpoint)?).await?;
        let requests = requests.await?;
        assert_eq!(requests.len(), 3);
        assert!(requests[0].starts_with("PATCH /reg/revoked-device "));
        assert!(requests[1].starts_with("POST /reg "));
        assert!(requests[2].starts_with("PATCH /reg/replacement-device "));
        assert_eq!(
            serde_json::to_value(account)?["registration"]["id"],
            "replacement-device"
        );
        assert!(store.load_pending().await?.is_none());
        tokio::fs::remove_file(root.join("secrets/masque-account.json")).await?;
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }

    #[tokio::test]
    async fn failed_account_commit_retains_pending_until_a_later_success() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-masque-commit-{}", nanoid::nanoid!()));
        let store = MasqueAccountStore::new(root.clone());
        let pending = PendingMasque::from_created(
            MasqueKeys::generate()?,
            br#"{"id":"device-1","token":"synthetic-token"}"#,
        )?;
        store.save_pending(&pending).await?;
        let before = tokio::fs::read(root.join("secrets/masque-pending.json")).await?;
        let peer = serde_json::to_value(MasqueKeys::generate()?)?["public_key"]
            .as_str()
            .unwrap()
            .to_owned();
        let enrollment = serde_json::json!({"config": {
            "peers":[{"public_key":format!("-----BEGIN PUBLIC KEY-----\n{peer}\n-----END PUBLIC KEY-----")}],
            "interface":{"addresses":{"v4":"172.16.0.2","v6":""}}
        }})
        .to_string();
        let blocked_root = root.clone();
        let (endpoint, requests) = server_before_reply(vec![Some((200, enrollment.clone()))], move || {
            std::fs::create_dir(blocked_root.join("secrets/masque-account.json")).unwrap();
        })
        .await;
        let registrar = HttpMasqueRegistrar::at_endpoint(&endpoint)?;
        let error = ensure_registration(&store, &registrar).await.err().unwrap();
        assert_eq!(error.to_string(), "Unable to save MASQUE account");
        assert_eq!(tokio::fs::read(root.join("secrets/masque-pending.json")).await?, before);
        assert!(requests.await?[0].starts_with("PATCH /reg/device-1 "));
        tokio::fs::remove_dir(root.join("secrets/masque-account.json")).await?;
        let (endpoint, requests) = server(vec![Some((200, enrollment))]).await;
        let registrar = HttpMasqueRegistrar::at_endpoint(&endpoint)?;
        ensure_registration(&store, &registrar).await?;
        assert!(requests.await?[0].starts_with("PATCH /reg/device-1 "));
        assert!(store.load_pending().await?.is_none());
        assert!(store.load().await?.is_some());
        tokio::fs::remove_file(root.join("secrets/masque-account.json")).await?;
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }

    #[tokio::test]
    async fn rejected_enrollment_keeps_pending_and_distinguishes_revocation_from_access_denial() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-masque-errors-{}", nanoid::nanoid!()));
        let store = MasqueAccountStore::new(root.clone());
        let pending = crate::config::warp_masque_account::PendingMasque::from_created(
            MasqueKeys::generate()?,
            br#"{"id":"device-1","token":"synthetic-token"}"#,
        )?;
        store.save_pending(&pending).await?;
        let before = tokio::fs::read(root.join("secrets/masque-pending.json")).await?;
        for (status, body, revoked) in [
            (401, "synthetic-secret-body".into(), true),
            (403, "synthetic-secret-body".into(), false),
            (404, "synthetic-secret-body".into(), true),
            (410, "synthetic-secret-body".into(), true),
            (429, "synthetic-secret-body".into(), false),
            (200, "synthetic-secret-body".into(), false),
            (200, "{}".into(), false),
            (200, "x".repeat(64 * 1024 + 1), false),
        ] {
            let (endpoint, requests) = server(vec![Some((status, body))]).await;
            let registrar = HttpMasqueRegistrar::at_endpoint(&endpoint)?;
            let error = ensure_registration(&store, &registrar).await.err().unwrap();
            assert_eq!(error.is::<UnusableMasqueRegistration>(), revoked, "status {status}");
            assert!(!format!("{error:#}").contains("synthetic-secret-body"));
            assert!(!format!("{error:#}").contains("synthetic-token"));
            assert!(requests.await?[0].starts_with("PATCH /reg/device-1 "));
            assert_eq!(tokio::fs::read(root.join("secrets/masque-pending.json")).await?, before);
            assert!(store.load().await?.is_none());
        }
        store.clear_pending().await?;
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }

    #[tokio::test]
    async fn patch_timeout_preserves_pending_identity_for_next_attempt() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-masque-timeout-{}", nanoid::nanoid!()));
        let store = MasqueAccountStore::new(root.clone());
        let pending = crate::config::warp_masque_account::PendingMasque::from_created(
            MasqueKeys::generate()?,
            br#"{"id":"device-1","token":"synthetic-token"}"#,
        )?;
        store.save_pending(&pending).await?;
        let before = tokio::fs::read(root.join("secrets/masque-pending.json")).await?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/reg", listener.local_addr()?);
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let mut bytes = [0u8; 1024];
                let n = stream.read(&mut bytes).await.unwrap();
                assert_ne!(n, 0);
                request.extend_from_slice(&bytes[..n]);
            }
            let _ = stopped.await;
            drop(stream);
            String::from_utf8(request).unwrap()
        });
        let mut registrar = HttpMasqueRegistrar::at_endpoint(&endpoint)?;
        registrar.client = reqwest::Client::builder()
            .no_proxy()
            .retry(reqwest::retry::never())
            .timeout(Duration::from_millis(150))
            .build()?;
        let error = ensure_registration(&store, &registrar).await.err().unwrap();
        let _ = stop.send(());
        let request = tokio::time::timeout(Duration::from_secs(3), task).await??;
        assert!(request.starts_with("PATCH /reg/device-1 "));
        assert_eq!(
            error.downcast_ref::<WarpFailure>().unwrap().reason,
            FailureReason::Timeout
        );
        assert_eq!(tokio::fs::read(root.join("secrets/masque-pending.json")).await?, before);
        assert!(store.load().await?.is_none());
        store.clear_pending().await?;
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }

    #[tokio::test]
    async fn lost_patch_response_resumes_same_identity_and_completed_account_skips_http() -> anyhow::Result<()> {
        let _production_client = HttpMasqueRegistrar::new()?;
        let root = std::env::temp_dir().join(format!("tropa-masque-http-{}", nanoid::nanoid!()));
        tokio::fs::create_dir_all(root.join("secrets")).await?;
        let wg = root.join("secrets/warp-account.json");
        tokio::fs::write(&wg, b"untouched-wg-marker").await?;
        let peer = serde_json::to_value(MasqueKeys::generate()?)?["public_key"]
            .as_str()
            .unwrap()
            .to_owned();
        let enrollment = serde_json::json!({"id":"ignored-replacement", "token":"ignored-token", "config": {
            "peers":[{"public_key": format!("-----BEGIN PUBLIC KEY-----\n{peer}\n-----END PUBLIC KEY-----")}],
            "interface":{"addresses":{"v4":"172.16.0.2", "v6":""}}
        }})
        .to_string();
        let (endpoint, requests) = server(vec![
            Some((200, r#"{"id":"device-1","token":"synthetic-token"}"#.into())),
            None,
        ])
        .await;
        let store = MasqueAccountStore::new(root.clone());
        let registrar = HttpMasqueRegistrar::at_endpoint(&endpoint)?;
        assert!(ensure_registration(&store, &registrar).await.is_err());
        let pending = serde_json::to_value(store.load_pending().await?.unwrap())?;
        let captured = requests.await?;
        assert_eq!(captured.len(), 2);
        assert!(captured[0].starts_with("POST /reg "));
        assert!(captured[1].starts_with("PATCH /reg/device-1 "));
        assert!(
            captured[1]
                .to_lowercase()
                .contains("authorization: bearer synthetic-token")
        );
        for request in &captured {
            assert!(request.to_lowercase().contains("cf-client-version: a-6.35-4471"));
            assert!(request.to_lowercase().contains("user-agent: warp for android"));
            assert!(!request.contains(pending["keys"]["private_key"].as_str().unwrap()));
        }
        let post: serde_json::Value = serde_json::from_str(captured[0].split("\r\n\r\n").nth(1).unwrap())?;
        assert_eq!(post["tunnel_type"], "wireguard");
        assert_eq!(post["key_type"], "curve25519");
        assert!(chrono::DateTime::parse_from_rfc3339(post["tos"].as_str().unwrap()).is_ok());
        let patch: serde_json::Value = serde_json::from_str(captured[1].split("\r\n\r\n").nth(1).unwrap())?;
        assert_eq!(patch["key"], pending["keys"]["public_key"]);
        assert_eq!(patch["key_type"], "secp256r1");
        assert_eq!(patch["tunnel_type"], "masque");
        let (endpoint, requests) = server(vec![Some((200, enrollment))]).await;
        let registrar = HttpMasqueRegistrar::at_endpoint(&endpoint)?;
        let restarted = MasqueAccountStore::new(root.clone());
        let account = ensure_registration(&restarted, &registrar).await?;
        let captured = requests.await?;
        assert!(captured[0].starts_with("PATCH /reg/device-1 "));
        assert_eq!(serde_json::to_value(&account)?["registration"], pending);
        assert!(store.load_pending().await?.is_none());
        let reused = ensure_registration(&restarted, &registrar).await?;
        assert_eq!(serde_json::to_value(reused)?, serde_json::to_value(account)?);
        assert_eq!(tokio::fs::read(&wg).await?, b"untouched-wg-marker");
        for name in ["warp-account.json", "masque-account.json"] {
            tokio::fs::remove_file(root.join("secrets").join(name)).await?;
        }
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }
}

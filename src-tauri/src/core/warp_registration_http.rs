use super::warp_failure::{FailureReason, FailureStage, WarpFailure};
use anyhow::{Result, ensure};
#[cfg(windows)]
use base64::{Engine as _, engine::general_purpose::STANDARD};
#[cfg(windows)]
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Default)]
pub(crate) struct RegistrationRoute {
    runtime_root: Option<std::path::PathBuf>,
    checked: Mutex<bool>,
    #[cfg(windows)]
    tunnel: Mutex<Option<super::warp_engine::process::RegistrationWorker>>,
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::{
        config::warp_account::WarpAccountStore,
        core::warp_registration::{HttpWarpRegistrar, ensure_registration},
    };
    use std::sync::Arc;

    struct BlockedDns;
    impl reqwest::dns::Resolve for BlockedDns {
        fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
            Box::pin(async { Err(std::io::Error::from(std::io::ErrorKind::NotConnected).into()) })
        }
    }

    #[tokio::test]
    #[ignore = "creates a real test device; requires a freshly built source-bound registration worker"]
    async fn blocked_direct_api_registers_and_persists_through_owned_worker() -> Result<()> {
        let runtime = std::path::PathBuf::from(std::env::var("TROPA_REGISTRATION_RUNTIME")?);
        let root = std::env::temp_dir().join(format!("tropa-bootstrap-{}", nanoid::nanoid!()));
        tokio::fs::create_dir(&root).await?;
        let _cleanup = scopeguard::guard(root.clone(), |path| {
            let _ = std::fs::remove_dir_all(path);
        });
        let route = Arc::new(RegistrationRoute::for_runtime(&runtime));
        let mut registrar = HttpWarpRegistrar::new()?.using_route(route.clone());
        registrar.client = reqwest::Client::builder()
            .no_proxy()
            .dns_resolver(Arc::new(BlockedDns))
            .build()?;
        let store = WarpAccountStore::new(root.clone());
        ensure_registration(&store, &registrar).await?;
        let before = tokio::fs::read(root.join("secrets/warp-account.json")).await?;
        assert!(!tokio::fs::try_exists(root.join("secrets/warp-pending.json")).await?);
        ensure_registration(&store, &registrar).await?;
        assert_eq!(before, tokio::fs::read(root.join("secrets/warp-account.json")).await?);
        let masque_store = crate::config::warp_masque_account::MasqueAccountStore::new(root.clone());
        let masque_registrar =
            crate::core::warp_masque_registration::HttpMasqueRegistrar::new()?.using_route(route.clone());
        crate::core::warp_masque_registration::ensure_registration(&masque_store, &masque_registrar).await?;
        assert!(masque_store.load().await?.is_some());
        assert_eq!(before, tokio::fs::read(root.join("secrets/warp-account.json")).await?);
        drop(masque_registrar);
        drop(registrar);
        drop(route);
        crate::core::warp_children::wait_for_exit().await?;
        Ok(())
    }
}

pub(crate) struct RegistrationResponse {
    pub(crate) status: u16,
    pub(crate) body: Vec<u8>,
}

fn failure(reason: FailureReason) -> anyhow::Error {
    WarpFailure::new(FailureStage::Registration, reason).into()
}

impl RegistrationRoute {
    pub(crate) fn for_runtime(root: &std::path::Path) -> Self {
        Self {
            runtime_root: Some(root.into()),
            ..Default::default()
        }
    }

    pub(crate) async fn send(
        &self,
        client: &reqwest::Client,
        request: reqwest::Request,
    ) -> Result<RegistrationResponse> {
        let cloudflare = request.url().scheme() == "https"
            && request.url().host_str() == Some("api.devices.cloudflare.com")
            && request.url().port_or_known_default() == Some(443);
        if cloudflare {
            let mut checked = self.checked.lock().await;
            #[cfg(windows)]
            {
                let mut tunnel = self.tunnel.lock().await;
                if tunnel.is_some() {
                    drop(checked);
                    return Box::pin(self.through_tunnel(&mut tunnel, &request)).await;
                }
                if !*checked {
                    let reachable = client
                        .get("https://api.devices.cloudflare.com/")
                        .timeout(Duration::from_secs(3))
                        .send()
                        .await
                        .is_ok();
                    *checked = true;
                    drop(checked);
                    if !reachable {
                        return Box::pin(self.through_tunnel(&mut tunnel, &request)).await;
                    }
                } else {
                    drop(checked);
                }
                drop(tunnel);
            }
            #[cfg(not(windows))]
            {
                *checked = true;
                drop(checked);
            }
        }
        let backup = request.try_clone();
        let response = client.execute(request).await;
        #[cfg(windows)]
        if cloudflare
            && response.as_ref().is_err_and(reqwest::Error::is_connect)
            && let Some(request) = backup
        {
            let mut tunnel = self.tunnel.lock().await;
            return Box::pin(self.through_tunnel(&mut tunnel, &request)).await;
        }
        #[cfg(not(windows))]
        let _ = backup;
        let mut response = response.map_err(|error| {
            WarpFailure::from_error(&error.into(), FailureStage::Registration, FailureReason::Unreachable)
        })?;
        let status = response.status().as_u16();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| {
            WarpFailure::from_error(&error.into(), FailureStage::Registration, FailureReason::InvalidOutput)
        })? {
            ensure!(
                body.len() + chunk.len() <= 64 * 1024,
                failure(FailureReason::InvalidOutput)
            );
            body.extend_from_slice(&chunk);
        }
        Ok(RegistrationResponse { status, body })
    }

    #[cfg(windows)]
    async fn through_tunnel(
        &self,
        tunnel: &mut Option<super::warp_engine::process::RegistrationWorker>,
        request: &reqwest::Request,
    ) -> Result<RegistrationResponse> {
        let exchange = async {
            let mut worker = if let Some(worker) = tunnel.take() {
                worker
            } else {
                let executable = std::env::current_exe()?;
                let root = self
                    .runtime_root
                    .as_deref()
                    .or_else(|| executable.parent())
                    .ok_or_else(|| failure(FailureReason::InvalidOutput))?;
                let artifact = super::warp_engine::process::WorkerArtifact::packaged(root)?;
                super::warp_engine::process::RegistrationWorker::start(root, artifact).await?
            };
            let body: serde_json::Value = serde_json::from_slice(
                request
                    .body()
                    .and_then(reqwest::Body::as_bytes)
                    .ok_or_else(|| failure(FailureReason::InvalidOutput))?,
            )?;
            let token = request
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "));
            let payload = serde_json::to_vec(&serde_json::json!({
                "method": request.method().as_str(), "path": request.url().path(),
                "token": token.unwrap_or_default(), "body": body,
            }))?;
            let data = worker.request(&payload).await?;
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Response {
                status: u16,
                body: String,
            }
            let response: Response = serde_json::from_slice(&data)?;
            let body = STANDARD.decode(response.body)?;
            ensure!(
                (100..=599).contains(&response.status) && body.len() <= 64 * 1024,
                failure(FailureReason::InvalidOutput)
            );
            *tunnel = Some(worker);
            Ok(RegistrationResponse {
                status: response.status,
                body,
            })
        }
        .await;
        if exchange.is_err() {
            tunnel.take();
        }
        exchange.map_err(|_| failure(FailureReason::Unreachable))
    }
}

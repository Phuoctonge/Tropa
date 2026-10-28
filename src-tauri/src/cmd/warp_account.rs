use super::{CmdResult, CommandFailure};
use crate::{
    config::{
        Config,
        warp_account::{WarpAccountStatus, WarpAccountStore},
    },
    core::warp_registration::{HttpWarpRegistrar, ensure_registration, reset_registration},
    utils::dirs,
};

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn get_warp_account_status() -> CmdResult<WarpAccountStatus> {
    async {
        let _write = Config::lock_config_write().await;
        WarpAccountStore::new(dirs::app_home_dir()?).status().await
    }
    .await
    .map_err(|_| CommandFailure::coded("WARP_ACCOUNT_READ_FAILED", "Unable to read WARP registration"))
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn ensure_warp_registration() -> CmdResult<WarpAccountStatus> {
    async {
        let _write = Config::lock_config_write().await;
        let _catalog_write = crate::config::warp_profiles::WarpCatalogLock::acquire(&dirs::app_home_dir()?)?;
        let store = WarpAccountStore::new(dirs::app_home_dir()?);
        ensure_registration(&store, &HttpWarpRegistrar::new()?).await
    }
    .await
    .map_err(|_| CommandFailure::coded("WARP_REGISTRATION_FAILED", "Unable to register WARP device"))
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn reset_warp_registration() -> CmdResult<WarpAccountStatus> {
    async {
        #[cfg(all(windows, feature = "portable"))]
        let _operation = crate::core::warp_connection::WARP_CONNECTION.idle_guard()?;
        let _write = Config::lock_config_write().await;
        let _catalog_write = crate::config::warp_profiles::WarpCatalogLock::acquire(&dirs::app_home_dir()?)?;
        anyhow::ensure!(
            matches!(
                *crate::core::CoreManager::global().get_running_mode(),
                crate::core::manager::RunningMode::NotRunning
            ),
            "Disconnect before resetting WARP registration"
        );
        let store = WarpAccountStore::new(dirs::app_home_dir()?);
        reset_registration(&store, &HttpWarpRegistrar::new()?).await
    }
    .await
    .map_err(|_| CommandFailure::coded("WARP_REGISTRATION_RESET_FAILED", "Unable to reset WARP registration"))
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn get_masque_account_status() -> CmdResult<crate::config::warp_masque_account::MasqueAccountStatus> {
    async {
        let _write = Config::lock_config_write().await;
        crate::config::warp_masque_account::MasqueAccountStore::new(dirs::app_home_dir()?)
            .status()
            .await
    }
    .await
    .map_err(|_| CommandFailure::coded("MASQUE_ACCOUNT_READ_FAILED", "Unable to read MASQUE registration"))
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn reset_masque_registration() -> CmdResult<crate::config::warp_masque_account::MasqueAccountStatus> {
    async {
        #[cfg(all(windows, feature = "portable"))]
        let _operation = crate::core::warp_connection::WARP_CONNECTION.idle_guard()?;
        let _write = Config::lock_config_write().await;
        let _catalog_write = crate::config::warp_profiles::WarpCatalogLock::acquire(&dirs::app_home_dir()?)?;
        anyhow::ensure!(
            matches!(
                *crate::core::CoreManager::global().get_running_mode(),
                crate::core::manager::RunningMode::NotRunning
            ),
            "Disconnect before resetting MASQUE registration"
        );
        let store = crate::config::warp_masque_account::MasqueAccountStore::new(dirs::app_home_dir()?);
        let account = crate::core::warp_masque_registration::reset_registration(
            &store,
            &crate::core::warp_masque_registration::HttpMasqueRegistrar::new()?,
        )
        .await?;
        anyhow::Ok(account.status())
    }
    .await
    .map_err(|_| {
        CommandFailure::coded(
            "MASQUE_REGISTRATION_RESET_FAILED",
            "Unable to reset MASQUE registration",
        )
    })
}

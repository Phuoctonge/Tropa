use super::{CoreManager, RunningMode};
use crate::{config::Config, core::handle::Handle};
use anyhow::{Context, Result, ensure};
use std::{future::Future, path::PathBuf, time::Duration};

async fn stop_runtime<Disable, DisableFuture, Stop, StopFuture, Delete, DeleteFuture>(
    disable: Disable,
    stop: Stop,
    delete: Delete,
) -> Result<()>
where
    Disable: FnOnce() -> DisableFuture,
    DisableFuture: Future<Output = Result<()>>,
    Stop: FnOnce() -> StopFuture,
    StopFuture: Future<Output = Result<()>>,
    Delete: FnOnce() -> DeleteFuture,
    DeleteFuture: Future<Output = Result<()>>,
{
    disable().await?;
    stop().await?;
    delete().await
}

impl CoreManager {
    pub(crate) fn owns_tropa_runtime(&self) -> bool {
        self.tropa_runtime.lock().is_some()
    }

    pub(crate) async fn start_tropa_core(&self, runtime: PathBuf) -> Result<()> {
        let write = Config::lock_config_write().await;
        let _update = self.claim_config_update(&write)?;
        let _life = self.lifecycle_lock.lock().await;
        ensure!(!Handle::global().is_exiting(), "Application is exiting");
        ensure!(
            crate::core::runstate::RUN_STATE.state().is_admin,
            "Administrator rights are required for TUN"
        );
        ensure!(
            !self.owns_tropa_runtime() && matches!(*self.get_running_mode(), RunningMode::NotRunning),
            "Another core is already owned"
        );
        ensure!(runtime.is_absolute() && runtime.is_file(), "Tropa runtime is missing");
        *self.tropa_runtime.lock() = Some(runtime);
        if let Err(error) = self.start_core_by_sidecar().await {
            // An unready child can still be terminating. Keep ownership until its death is confirmed.
            self.wait_for_sidecar_exit().await?;
            self.remove_tropa_runtime().await?;
            return Err(error);
        }
        Ok(())
    }

    pub(crate) async fn set_tropa_tun(&self, enabled: bool) -> Result<()> {
        ensure!(self.owns_tropa_runtime(), "Tropa does not own this core");
        tokio::time::timeout(
            Duration::from_secs(5),
            Handle::mihomo().patch_base_config(&serde_json::json!({"tun":{"enable":enabled}})),
        )
        .await
        .context("TUN update timed out")?
        .map_err(|_| anyhow::anyhow!("Unable to change Tropa TUN state"))
    }

    async fn remove_tropa_runtime(&self) -> Result<()> {
        let path = self.tropa_runtime.lock().clone();
        if let Some(path) = path {
            match tokio::fs::remove_file(path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => anyhow::bail!("Unable to remove temporary Tropa runtime"),
            }
        }
        *self.tropa_runtime.lock() = None;
        Ok(())
    }

    pub(super) async fn stop_tropa_core_inner(&self) -> Result<()> {
        stop_runtime(
            || async {
                if self.get_running_sidecar_pid().is_some() {
                    self.set_tropa_tun(false).await?;
                }
                Ok(())
            },
            || async {
                self.stop_core_by_sidecar_unprepared().await?;
                Ok(())
            },
            || self.remove_tropa_runtime(),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[tokio::test]
    async fn stop_does_not_kill_or_delete_runtime_when_tun_disable_fails() {
        let steps = RefCell::new(Vec::new());
        let result = stop_runtime(
            || async {
                steps.borrow_mut().push("disable");
                anyhow::bail!("IPC unavailable")
            },
            || async {
                steps.borrow_mut().push("stop");
                Ok(())
            },
            || async {
                steps.borrow_mut().push("delete");
                Ok(())
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(*steps.borrow(), ["disable"]);
    }

    #[tokio::test]
    async fn runtime_is_deleted_only_after_confirmed_exit() {
        let steps = RefCell::new(Vec::new());
        let result = stop_runtime(
            || async {
                steps.borrow_mut().push("disable");
                Ok(())
            },
            || async {
                steps.borrow_mut().push("stop");
                anyhow::bail!("still running")
            },
            || async {
                steps.borrow_mut().push("delete");
                Ok(())
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(*steps.borrow(), ["disable", "stop"]);
    }

    #[tokio::test]
    async fn graceful_stop_disables_tun_before_termination_and_secret_cleanup() {
        let steps = RefCell::new(Vec::new());
        stop_runtime(
            || async {
                steps.borrow_mut().push("disable");
                Ok(())
            },
            || async {
                steps.borrow_mut().push("stop");
                Ok(())
            },
            || async {
                steps.borrow_mut().push("delete");
                Ok(())
            },
        )
        .await
        .unwrap();
        assert_eq!(*steps.borrow(), ["disable", "stop", "delete"]);
    }
}

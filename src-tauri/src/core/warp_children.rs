use anyhow::{Result, ensure};
use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::{Arc, LazyLock},
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{ERROR_INVALID_PARAMETER, WAIT_OBJECT_0},
    System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
};

static CHILDREN: LazyLock<parking_lot::Mutex<Vec<tokio::task::JoinHandle<()>>>> = LazyLock::new(Default::default);

fn supervise<T: Send + Sync + 'static>(pid: u32, owner: Arc<T>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let handle = loop {
            let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
            if !raw.is_null() {
                break Some(unsafe { OwnedHandle::from_raw_handle(raw) });
            }
            if std::io::Error::last_os_error().raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
                break None;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        if let Some(handle) = handle {
            while unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) } != WAIT_OBJECT_0 {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        drop(owner);
    })
}

pub(crate) fn track<T: Send + Sync + 'static>(pid: u32, owner: Arc<T>) {
    let mut children = CHILDREN.lock();
    children.retain(|child| !child.is_finished());
    children.push(supervise(pid, owner));
}

pub(crate) fn has_pending() -> bool {
    let mut children = CHILDREN.lock();
    children.retain(|child| !child.is_finished());
    !children.is_empty()
}

pub(crate) async fn wait_for_exit() -> Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while has_pending() {
        ensure!(
            tokio::time::Instant::now() < deadline,
            "Test process shutdown could not be confirmed; retry disconnect before exit"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn tracked_runtime_file_survives_caller_drop_until_os_confirms_child_exit() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!("tropa-runtime-lease-{}.json", nanoid::nanoid!()));
        std::fs::write(&path, b"synthetic-runtime-marker")?;
        let owner = Arc::new(scopeguard::guard(path.clone(), |path| {
            let _ = std::fs::remove_file(path);
        }));
        let mut child = tokio::process::Command::new("ping")
            .args(["-n", "999", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        track(child.id().unwrap(), Arc::clone(&owner));
        drop(owner);
        tokio::task::yield_now().await;
        assert_eq!(std::fs::read(&path)?, b"synthetic-runtime-marker");
        child.kill().await?;
        wait_for_exit().await?;
        assert!(!path.exists());
        Ok(())
    }

    #[tokio::test]
    async fn unconfirmed_children_keep_ownership_until_the_os_reports_exit() -> anyhow::Result<()> {
        let mut child = tokio::process::Command::new("ping")
            .args(["-n", "999", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let owner = Arc::new(());
        let weak = Arc::downgrade(&owner);
        let monitor = supervise(child.id().unwrap(), owner);
        assert!(!monitor.is_finished());
        assert!(weak.upgrade().is_some());
        child.kill().await?;
        tokio::time::timeout(std::time::Duration::from_secs(2), monitor).await??;
        assert!(weak.upgrade().is_none());
        Ok(())
    }
}

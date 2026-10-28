use anyhow::{Context, Result, ensure};
use std::{os::windows::ffi::OsStrExt, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, WAIT_OBJECT_0},
    System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
};

fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

pub(crate) fn request_relaunch() -> Result<()> {
    let executable = std::env::current_exe()?;
    let parent = executable.parent().context("Executable directory is missing")?;
    let file = wide(executable.as_os_str());
    let directory = wide(parent.as_os_str());
    let verb = wide(std::ffi::OsStr::new("runas"));
    let args = wide(std::ffi::OsStr::new(&format!(
        "--tropa-connect --tropa-wait-pid {}",
        std::process::id()
    )));
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            args.as_ptr(),
            directory.as_ptr(),
            SW_SHOWNORMAL,
        )
    };
    ensure!(
        result as isize > 32,
        "Windows did not authorize the administrator restart"
    );
    Ok(())
}

pub(crate) fn wait_for_parent() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let Some(index) = args.iter().position(|arg| arg == "--tropa-wait-pid") else {
        return Ok(());
    };
    let pid: u32 = args.get(index + 1).context("Parent PID is missing")?.parse()?;
    ensure!(pid != 0 && pid != std::process::id(), "Invalid parent PID");
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if process.is_null() {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(87) {
            return Ok(());
        }
        return Err(error.into());
    }
    let result = unsafe { WaitForSingleObject(process, 30_000) };
    unsafe {
        CloseHandle(process);
    }
    ensure!(result == WAIT_OBJECT_0, "Previous Tropa instance has not exited");
    Ok(())
}

pub(crate) fn auto_connect_requested() -> bool {
    std::env::args().any(|arg| arg == "--tropa-connect")
}

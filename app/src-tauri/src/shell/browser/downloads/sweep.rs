//! Clearing `.incoming/` at startup. Nothing staged outlives the copy of the
//! app that staged it, so a folder there is left by a crash — unless another
//! running copy owns it: folders are `<pid>-<id>`, and a live pid other than
//! ours is skipped.

use std::path::Path;

/// Empties `root` of everything no other running copy of the app owns.
pub(in crate::shell::browser) fn sweep_incoming(root: &Path) {
    sweep(root, std::process::id(), alive);
}

/// `sweep_incoming` with our pid and the liveness test given.
pub(super) fn sweep(root: &Path, own: u32, alive: impl Fn(u32) -> bool) {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            elog!("[oculus] browser: could not read the download staging folder: {e}");
            return;
        }
    };
    for entry in entries.flatten() {
        if owned_elsewhere(&entry.file_name().to_string_lossy(), own, &alive) {
            continue;
        }
        let path = entry.path();
        let removed = if entry.file_type().is_ok_and(|t| t.is_dir()) {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if let Err(e) = removed {
            elog!("[oculus] browser: could not clear a staged download: {e}");
        }
    }
}

/// Whether `<pid>-<id>` belongs to a live process that is not us.
fn owned_elsewhere(name: &str, own: u32, alive: &impl Fn(u32) -> bool) -> bool {
    name.split_once('-')
        .and_then(|(pid, _)| pid.parse::<u32>().ok())
        .is_some_and(|pid| pid != own && alive(pid))
}

/// Signal 0 checks without signalling; a process of another user (EPERM) is
/// alive too. A reused pid only keeps a stale folder until a later start.
#[cfg(unix)]
pub(super) fn alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // SAFETY: signal 0 delivers nothing.
    let reached = unsafe { libc::kill(pid, 0) } == 0;
    reached || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
pub(super) fn alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // `STILL_ACTIVE`, the exit code of a process that has not exited.
    const RUNNING: u32 = 259;

    // SAFETY: the handle is checked before use and closed after.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code) != 0;
        CloseHandle(handle);
        ok && code == RUNNING
    }
}

#[cfg(not(any(unix, windows)))]
pub(super) fn alive(_pid: u32) -> bool {
    false
}

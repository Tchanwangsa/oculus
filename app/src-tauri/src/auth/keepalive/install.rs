//! Installing the agent without being asked: the startup repair and the
//! install after a first working headless sign-in.

use super::commands::keepalive_enable;
use super::plist::{cli_path, interval_from_plist, plist_path, program_from_plist};
use super::DEFAULT_INTERVAL_HOURS;

/// Set when the user turns the agent off, so [`ensure_installed`] does not put
/// it straight back on the next automated sign-in.
pub(super) fn opt_out_path() -> std::path::PathBuf {
    crate::library::paths::data_dir().join("keepalive-disabled")
}

/// On startup, re-point an installed agent whose binary has moved (the plist
/// holds an absolute path). Never installs one that is not already there.
pub fn repair_path() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let Some(plist) = plist_path().filter(|p| p.exists()) else {
        return;
    };
    let Ok(cli) = cli_path() else {
        return;
    };
    let cli = cli.to_string_lossy().to_string();
    if program_from_plist(&plist).as_deref() == Some(cli.as_str()) {
        return;
    }

    let hours = interval_from_plist(&plist);
    std::thread::spawn(
        move || match tauri::async_runtime::block_on(keepalive_enable(hours)) {
            Ok(()) => eprintln!("[oculus] keep-alive agent re-pointed at {cli}"),
            Err(e) => eprintln!("[oculus] could not re-point keep-alive agent: {e}"),
        },
    );
}

/// Install the agent after a headless sign-in has actually worked — not merely
/// when credentials are stored: Okta Verify push / WebAuthn-only accounts cannot
/// be driven from a LaunchAgent. Called on the success path of
/// `crate::auth::okta::commands::signed_in`; silent in every early return.
pub fn ensure_installed() {
    if !cfg!(target_os = "macos") {
        return;
    }
    if opt_out_path().exists() {
        return;
    }
    if plist_path().is_some_and(|p| p.exists()) {
        return;
    }

    std::thread::spawn(move || {
        match tauri::async_runtime::block_on(keepalive_enable(DEFAULT_INTERVAL_HOURS)) {
            Ok(()) => eprintln!("[oculus] keep-alive agent installed automatically"),
            Err(e) => eprintln!("[oculus] could not install keep-alive agent: {e}"),
        }
    });
}

//! Tauri commands for the keep-alive agent.

use super::install::opt_out_path;
#[cfg(target_os = "macos")]
use super::launchctl::{gui_domain, launchctl};
use super::plist::{cli_path, interval_from_plist, plist_body, plist_path};
use super::status::{last_log_line, KeepaliveStatus};
use super::DEFAULT_INTERVAL_HOURS;

#[tauri::command]
pub fn keepalive_status() -> KeepaliveStatus {
    let supported = cfg!(target_os = "macos");
    let plist = plist_path();
    let enabled = supported && plist.as_ref().is_some_and(|p| p.exists());
    let interval_hours = match (&plist, enabled) {
        (Some(p), true) => interval_from_plist(p),
        _ => DEFAULT_INTERVAL_HOURS,
    };

    KeepaliveStatus {
        supported,
        enabled,
        interval_hours,
        last_run: if enabled { last_log_line() } else { None },
    }
}

/// Install (or re-install, to change the interval) the LaunchAgent.
#[tauri::command]
pub async fn keepalive_enable(interval_hours: u32) -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = interval_hours;
        Err("Background keep-alive is macOS-only.".to_string())
    }

    #[cfg(target_os = "macos")]
    {
        if crate::auth::saved_cookie_header().is_empty() {
            return Err("No saved session to keep alive — connect to Canvas first.".to_string());
        }

        let hours = interval_hours.clamp(1, 24);
        let plist = plist_path().ok_or("no HOME directory")?;
        let cli = cli_path()?;

        // An explicit enable is consent; drop any earlier opt-out.
        std::fs::remove_file(opt_out_path()).ok();
        // Remove the legacy shell-script agent, if any.
        std::fs::remove_file(crate::library::paths::data_dir().join("session-keepalive.sh")).ok();

        if let Some(dir) = plist.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&plist, plist_body(&cli.to_string_lossy(), hours * 3600))
            .map_err(|e| format!("could not write LaunchAgent: {e}"))?;

        let domain = gui_domain()?;
        let plist_str = plist.to_string_lossy().to_string();
        // Bootout any existing agent; the error when none was loaded is fine.
        let _ = launchctl(&["bootout", &domain, &plist_str]);
        launchctl(&["bootstrap", &domain, &plist_str]).map_err(|e| {
            // Leave no half-installed agent behind.
            std::fs::remove_file(&plist).ok();
            format!("launchctl refused the agent: {e}")
        })?;

        eprintln!("[oculus] keep-alive agent installed ({hours}h)");
        Ok(())
    }
}

#[tauri::command]
pub async fn keepalive_disable() -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(plist) = plist_path() {
            if plist.exists() {
                let plist_str = plist.to_string_lossy().to_string();
                if let Ok(domain) = gui_domain() {
                    let _ = launchctl(&["bootout", &domain, &plist_str]);
                }
                std::fs::remove_file(&plist).map_err(|e| e.to_string())?;
            }
        }
        std::fs::remove_file(crate::library::paths::data_dir().join("session-keepalive.sh")).ok();
        std::fs::write(opt_out_path(), b"1").ok();
        eprintln!("[oculus] keep-alive agent removed");
        Ok(())
    }
}

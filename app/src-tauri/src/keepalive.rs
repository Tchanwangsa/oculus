//! Background session keep-alive via a macOS LaunchAgent.
//!
//! The Canvas session is extended on use, so a periodic ping keeps it alive.
//! The in-app timer in `lib.rs` covers "Oculus is open"; launchd covers the
//! rest by running `oculus auth tick`, which shares the probe, cookie merge and
//! headless sign-in with the app. See `docs/auth.md`.

pub const LABEL: &str = "com.tchan.oculus.session-keepalive";
const DEFAULT_INTERVAL_HOURS: u32 = 6;

#[derive(serde::Serialize)]
pub struct KeepaliveStatus {
    /// False on platforms with no LaunchAgent support.
    pub supported: bool,
    pub enabled: bool,
    pub interval_hours: u32,
    /// Last line the agent logged, so the UI can show it is actually running.
    pub last_run: Option<String>,
}

/// The `oculus` CLI the agent runs: a sibling of the app executable (Tauri's
/// bundler copies every cargo bin into `Contents/MacOS/`), else the release build.
fn cli_path() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate the app binary: {e}"))?;
    let dir = exe
        .parent()
        .ok_or("the app binary has no parent directory")?;

    for candidate in [dir.join("oculus"), dir.join("../release/oculus")] {
        if candidate.is_file() {
            return candidate
                .canonicalize()
                .map_err(|e| format!("cannot resolve {}: {e}", candidate.display()));
        }
    }
    Err(format!(
        "the oculus CLI is not next to the app ({}) — run `cargo build --release --bin oculus`",
        dir.display()
    ))
}

/// Set when the user turns the agent off, so [`ensure_installed`] does not put
/// it straight back on the next automated sign-in.
fn opt_out_path() -> std::path::PathBuf {
    crate::paths::data_dir().join("keepalive-disabled")
}

fn plist_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::PathBuf::from(home)
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist")),
    )
}

fn plist_body(cli: &str, interval_secs: u32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{cli}</string>
        <string>auth</string>
        <string>tick</string>
    </array>
    <key>StartInterval</key>
    <integer>{interval_secs}</integer>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
"#
    )
}

#[cfg(target_os = "macos")]
fn gui_domain() -> Result<String, String> {
    let out = std::process::Command::new("id")
        .arg("-u")
        .output()
        .map_err(|e| format!("could not determine uid: {e}"))?;
    let uid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if uid.is_empty() {
        return Err("could not determine uid".to_string());
    }
    Ok(format!("gui/{uid}"))
}

#[cfg(target_os = "macos")]
fn launchctl(args: &[&str]) -> Result<(), String> {
    let out = std::process::Command::new("launchctl")
        .args(args)
        .output()
        .map_err(|e| format!("launchctl: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
}

fn last_log_line() -> Option<String> {
    let text = std::fs::read_to_string(crate::paths::keepalive_log_path(&crate::paths::data_dir()))
        .ok()?;
    text.lines().last().map(str::to_string)
}

/// The first `<string>` inside `ProgramArguments`. Scanned, not parsed.
fn program_from_plist(path: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let after = text.split("<key>ProgramArguments</key>").nth(1)?;
    let open = after.find("<string>")?;
    let rest = &after[open + "<string>".len()..];
    let close = rest.find("</string>")?;
    Some(rest[..close].trim().to_string())
}

fn interval_from_plist(path: &std::path::Path) -> u32 {
    let Ok(text) = std::fs::read_to_string(path) else {
        return DEFAULT_INTERVAL_HOURS;
    };
    let Some(after) = text.split("<key>StartInterval</key>").nth(1) else {
        return DEFAULT_INTERVAL_HOURS;
    };
    let Some(open) = after.find("<integer>") else {
        return DEFAULT_INTERVAL_HOURS;
    };
    let rest = &after[open + "<integer>".len()..];
    let Some(close) = rest.find("</integer>") else {
        return DEFAULT_INTERVAL_HOURS;
    };
    rest[..close]
        .trim()
        .parse::<u32>()
        .map(|secs| (secs / 3600).max(1))
        .unwrap_or(DEFAULT_INTERVAL_HOURS)
}

// ── Tauri commands ───────────────────────────────────────────────────────────

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
        std::fs::remove_file(crate::paths::data_dir().join("session-keepalive.sh")).ok();

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
        std::fs::remove_file(crate::paths::data_dir().join("session-keepalive.sh")).ok();
        std::fs::write(opt_out_path(), b"1").ok();
        eprintln!("[oculus] keep-alive agent removed");
        Ok(())
    }
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
/// [`crate::okta::run_sign_in`]; silent in every early return.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Emits the real generated plist so launchd's view of it can be checked
    /// outside the app. Set OCULUS_DUMP_AGENT=<dir>; otherwise this is a no-op.
    #[test]
    fn dump_agent_artifacts() {
        let Some(dir) = std::env::var_os("OCULUS_DUMP_AGENT") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::write(
            dir.join("agent.plist"),
            plist_body("/usr/local/bin/oculus", 6 * 3600),
        )
        .unwrap();
    }

    #[test]
    fn interval_round_trips_through_the_plist() {
        let dir = crate::test_support::Scratch::new("plist");
        let p = dir.join("t.plist");
        std::fs::write(&p, plist_body("/tmp/oculus", 6 * 3600)).unwrap();
        assert_eq!(interval_from_plist(&p), 6);
        std::fs::write(&p, "not a plist").unwrap();
        assert_eq!(interval_from_plist(&p), DEFAULT_INTERVAL_HOURS);
    }

    #[test]
    fn the_program_path_round_trips_through_the_plist() {
        let dir = crate::test_support::Scratch::new("plist");
        let p = dir.join("prog.plist");
        std::fs::write(
            &p,
            plist_body("/Applications/Oculus.app/Contents/MacOS/oculus", 3600),
        )
        .unwrap();
        assert_eq!(
            program_from_plist(&p).as_deref(),
            Some("/Applications/Oculus.app/Contents/MacOS/oculus")
        );
        std::fs::write(&p, "not a plist").unwrap();
        assert_eq!(program_from_plist(&p), None);
    }

    /// launchd execs ProgramArguments directly — no shell — so the CLI must be
    /// argv[0] with its subcommand as separate arguments, not one string.
    #[test]
    fn the_agent_invokes_the_cli_not_a_shell() {
        let plist = plist_body("/Applications/Oculus.app/Contents/MacOS/oculus", 6 * 3600);
        assert!(plist.contains("<string>/Applications/Oculus.app/Contents/MacOS/oculus</string>"));
        assert!(plist.contains("<string>auth</string>"));
        assert!(plist.contains("<string>tick</string>"));
        assert!(!plist.contains("/bin/sh"));
    }
}

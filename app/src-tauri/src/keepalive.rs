//! Background session keep-alive via launchd or Windows Task Scheduler.
//!
//! The Canvas session has no cookie-side expiry — the server tracks it and
//! extends it on use — so a periodic ping keeps it alive, but only while
//! something is making requests. The in-app timer in `lib.rs` covers "Oculus is
//! open"; this covers the rest by handing the same job to launchd, which runs
//! whether or not the app is.
//!
//! The agent runs the `oculus` CLI (`oculus auth tick`), not a shell script.
//! An earlier version was a standalone `/bin/sh` script that re-implemented the
//! cookie merge in awk, and it could only ping: when Canvas answered 401 it had
//! no way to reach Okta, so it logged the failure and gave up. Weeks of
//! `ping failed: HTTP 401` with nothing to show for them is what a
//! keep-alive that cannot re-authenticate actually looks like. Calling the
//! binary means the agent shares one implementation of the probe, the cookie
//! merge, and headless sign-in with the app.
//!
//! Still true: this cannot beat an absolute session cap or an SSO policy that
//! forces re-auth at a real browser. It also cannot answer Okta Verify push or
//! WebAuthn — automated recovery needs the TOTP factor, which is why the agent
//! installs itself only after a headless sign-in has actually succeeded.

use tauri::{AppHandle, Manager};

#[cfg(any(not(target_os = "windows"), test))]
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

/// The `oculus` CLI, which is what the agent actually runs.
///
/// Tauri's macOS bundler copies every cargo binary in this crate into
/// `Oculus.app/Contents/MacOS/`, so in a real install the CLI is a sibling of
/// the running app executable. In `tauri dev` the sibling is
/// `target/debug/oculus`, which exists only after `cargo build --bin oculus` —
/// hence the release fallback, and the explicit error rather than a plist
/// pointing at nothing.
fn cli_path() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate the app binary: {e}"))?;
    let dir = exe
        .parent()
        .ok_or("the app binary has no parent directory")?;

    let name = format!("oculus{}", std::env::consts::EXE_SUFFIX);
    for candidate in [dir.join(&name), dir.join("../release").join(&name)] {
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
fn opt_out_path(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .expect("no app data dir")
        .join("keepalive-disabled")
}

#[cfg(not(target_os = "windows"))]
fn plist_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::PathBuf::from(home)
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist")),
    )
}

#[cfg(any(target_os = "macos", test))]
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

fn last_log_line(app: &AppHandle) -> Option<String> {
    let dir = app.path().app_data_dir().ok()?;
    let text = std::fs::read_to_string(crate::paths::keepalive_log_path(&dir)).ok()?;
    text.lines().last().map(str::to_string)
}

/// The binary an installed agent is pointing at — the first `<string>` inside
/// `ProgramArguments`. Small enough to scan rather than pull in a plist parser,
/// the same way the interval is read back.
#[cfg(any(target_os = "macos", test))]
fn program_from_plist(path: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let after = text.split("<key>ProgramArguments</key>").nth(1)?;
    let open = after.find("<string>")?;
    let rest = &after[open + "<string>".len()..];
    let close = rest.find("</string>")?;
    Some(rest[..close].trim().to_string())
}

#[cfg(any(not(target_os = "windows"), test))]
fn interval_from_plist(path: &std::path::Path) -> u32 {
    let Ok(text) = std::fs::read_to_string(path) else {
        return DEFAULT_INTERVAL_HOURS;
    };
    // Small enough to scan rather than pull in a plist parser.
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
pub async fn keepalive_status(app: AppHandle) -> KeepaliveStatus {
    #[cfg(target_os = "windows")]
    {
        let state = windows_schedule::query().ok().flatten();
        let enabled = state.as_ref().is_some_and(|s| s.enabled);
        return KeepaliveStatus {
            supported: true,
            enabled,
            interval_hours: state.map(|s| s.hours).unwrap_or(DEFAULT_INTERVAL_HOURS),
            last_run: if enabled { last_log_line(&app) } else { None },
        };
    }
    #[cfg(not(target_os = "windows"))]
    {
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
        last_run: if enabled { last_log_line(&app) } else { None },
    }
    }
}

/// Install (or re-install, to change the interval) the LaunchAgent.
#[tauri::command]
pub async fn keepalive_enable(app: AppHandle, interval_hours: u32) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if crate::auth::saved_cookie_header(&app).is_empty() {
            return Err("No saved session to keep alive — connect to Canvas first.".into());
        }
        windows_schedule::install(&cli_path()?, interval_hours.clamp(1, 24))?;
        std::fs::remove_file(opt_out_path(&app)).ok();
        return Ok(());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (app, interval_hours);
        Err("Background keep-alive is available on Windows and macOS.".to_string())
    }

    #[cfg(target_os = "macos")]
    {
        // A token needs no keep-alive, and an empty cookie file means there is
        // nothing to keep alive yet.
        if crate::auth::saved_cookie_header(&app).is_empty() {
            return Err("No saved session to keep alive — connect to Canvas first.".to_string());
        }

        let hours = interval_hours.clamp(1, 24);
        let plist = plist_path().ok_or("no HOME directory")?;
        let cli = cli_path()?;

        // An explicit enable is consent; drop any earlier opt-out.
        std::fs::remove_file(opt_out_path(&app)).ok();
        // Sweep the shell agent this replaced, so no stale script is left in
        // the data dir looking like it is still doing something.
        std::fs::remove_file(
            app.path()
                .app_data_dir()
                .map_err(|e| e.to_string())?
                .join("session-keepalive.sh"),
        )
        .ok();

        if let Some(dir) = plist.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&plist, plist_body(&cli.to_string_lossy(), hours * 3600))
            .map_err(|e| format!("could not write LaunchAgent: {e}"))?;

        let domain = gui_domain()?;
        let plist_str = plist.to_string_lossy().to_string();
        // Replacing an existing agent: bootout first, and ignore the error when
        // nothing was loaded.
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
pub async fn keepalive_disable(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        windows_schedule::remove()?;
        let marker = opt_out_path(&app);
        if let Some(dir) = marker.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
        std::fs::write(marker, b"1").map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
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
        if let Ok(dir) = app.path().app_data_dir() {
            std::fs::remove_file(dir.join("session-keepalive.sh")).ok();
        }
        // Remember the choice: an automated sign-in must not reinstall an agent
        // the user has just switched off.
        std::fs::write(opt_out_path(&app), b"1").ok();
        eprintln!("[oculus] keep-alive agent removed");
        Ok(())
    }
}

/// Re-point an installed agent whose binary has moved.
///
/// The plist stores an absolute path, so dragging Oculus from Downloads to
/// Applications — or any update that replaces the bundle at a new path — leaves
/// launchd calling a binary that is gone. It would keep "running" every six
/// hours and do nothing, which is indistinguishable from working right up until
/// the session dies. Runs on startup; never installs an agent that is not
/// already there, so it cannot override the user's choice either way.
pub fn repair_path(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    {
        let app = app.clone();
        std::thread::spawn(move || {
            let Ok(Some(state)) = windows_schedule::query() else { return };
            if !state.enabled || opt_out_path(&app).exists() { return; }
            let Ok(cli) = cli_path() else { return };
            if state.cli == cli.to_string_lossy() { return; }
            if let Err(e) = windows_schedule::install(&cli, state.hours) {
                eprintln!("[oculus] could not repair keep-alive task: {e}");
            }
        });
    }
    #[cfg(target_os = "macos")]
    {
    let Some(plist) = plist_path().filter(|p| p.exists()) else {
        return;
    };
    let Ok(cli) = cli_path() else {
        // In `tauri dev` the release CLI often is not built. Nothing to repair
        // to, and no reason to tear down a working agent over it.
        return;
    };
    let cli = cli.to_string_lossy().to_string();
    if program_from_plist(&plist).as_deref() == Some(cli.as_str()) {
        return;
    }

    let hours = interval_from_plist(&plist);
    let handle = app.clone();
    std::thread::spawn(move || {
        match tauri::async_runtime::block_on(keepalive_enable(handle, hours)) {
            Ok(()) => eprintln!("[oculus] keep-alive agent re-pointed at {cli}"),
            Err(e) => eprintln!("[oculus] could not re-point keep-alive agent: {e}"),
        }
    });
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;
}

/// Install the agent after a headless sign-in has actually worked.
///
/// The gate is deliberately "a real automated sign-in just succeeded", not "the
/// user has credentials stored": automated recovery needs the TOTP factor, and
/// an Okta policy that answers only with Verify push or WebAuthn cannot be
/// driven from a LaunchAgent. On those accounts the agent would wake every six
/// hours, fail, and log noise forever — so it is never installed. Callers
/// invoke this only on the success path of [`crate::okta::run_sign_in`].
///
/// Silent by design in every early return: this runs behind a sign-in the user
/// asked for, and none of these cases are errors in *that* operation.
pub fn ensure_installed(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    {
        if opt_out_path(app).exists() { return; }
        let handle = app.clone();
        std::thread::spawn(move || {
            match windows_schedule::query() {
                Ok(None) => {
                    if let Err(e) = tauri::async_runtime::block_on(keepalive_enable(handle, DEFAULT_INTERVAL_HOURS)) {
                        eprintln!("[oculus] could not install keep-alive task: {e}");
                    }
                }
                Ok(Some(_)) => {},
                Err(e) => eprintln!("[oculus] could not inspect keep-alive task: {e}"),
            }
        });
    }
    #[cfg(target_os = "macos")]
    {
    if opt_out_path(app).exists() {
        return;
    }
    if plist_path().is_some_and(|p| p.exists()) {
        return;
    }

    let handle = app.clone();
    // launchctl and the plist write are both blocking; the sign-in that calls
    // this is already off the UI thread, but keep it off the caller's latency.
    std::thread::spawn(move || {
        match tauri::async_runtime::block_on(keepalive_enable(
            handle,
            DEFAULT_INTERVAL_HOURS,
        )) {
            Ok(()) => eprintln!("[oculus] keep-alive agent installed automatically"),
            Err(e) => eprintln!("[oculus] could not install keep-alive agent: {e}"),
        }
    });
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;
}

/// Scheduler commands are fixed scripts; paths and intervals travel as
/// environment values, never source interpolation. Each Windows account owns
/// its own limited, interactive-logon task, with no stored Windows password.
#[cfg(target_os = "windows")]
mod windows_schedule {
    use std::path::Path;

    #[derive(serde::Deserialize)]
    pub struct Installed {
        pub enabled: bool,
        pub cli: String,
        pub hours: u32,
    }

    const PREAMBLE: &str = r#"
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
$taskName = 'com.tchan.oculus.session-keepalive-' + $identity.User.Value
if ($env:OCULUS_SCHEDULER_TEST_NAME) { $taskName = $env:OCULUS_SCHEDULER_TEST_NAME }
"#;

    fn run(script: &str, cli: Option<&Path>, hours: u32, test_name: Option<&str>) -> Result<String, String> {
        let system = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        let powershell = std::path::PathBuf::from(system).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut command = crate::platform::command(powershell);
        command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command"])
            .arg(format!("{PREAMBLE}\n{script}"))
            .env("OCULUS_TASK_HOURS", hours.to_string())
            // Always overwrite inherited values: production can never be
            // redirected to a test task by the launching shell.
            .env("OCULUS_SCHEDULER_TEST_NAME", test_name.unwrap_or(""));
        if let Some(cli) = cli { command.env("OCULUS_TASK_CLI", cli); }
        let output = command.output().map_err(|e| format!("Windows Task Scheduler: {e}"))?;
        if !output.status.success() {
            return Err(format!("Windows Task Scheduler: {}", String::from_utf8_lossy(&output.stderr).trim()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().trim_start_matches('\u{feff}').to_string())
    }

    pub fn query() -> Result<Option<Installed>, String> {
        query_named(None)
    }

    fn query_named(test_name: Option<&str>) -> Result<Option<Installed>, String> {
        let json = run(r#"
$task = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
if ($null -eq $task) { exit 0 }
$meta = $task.Description | ConvertFrom-Json
[ordered]@{ enabled = ($task.State -ne 'Disabled'); cli = $meta.cli; hours = [int]$meta.hours } | ConvertTo-Json -Compress
"#, None, 0, test_name)?;
        if json.is_empty() { return Ok(None); }
        serde_json::from_str(&json).map(Some).map_err(|e| format!("Invalid keep-alive task metadata: {e}"))
    }

    pub fn install(cli: &Path, hours: u32) -> Result<(), String> {
        install_named(cli, hours, None)
    }

    fn install_named(cli: &Path, hours: u32, test_name: Option<&str>) -> Result<(), String> {
        run(r#"
$cli = $env:OCULUS_TASK_CLI
$hours = [int]$env:OCULUS_TASK_HOURS
$quoted = "'" + $cli.Replace("'", "''") + "'"
$invoke = '& ' + $quoted + ' auth tick'
if ($env:OCULUS_SCHEDULER_TEST_NAME) { $invoke = 'exit 0' }
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($invoke))
$action = New-ScheduledTaskAction -Execute (Join-Path $PSHOME 'powershell.exe') -Argument ('-NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand ' + $encoded)
$trigger = New-ScheduledTaskTrigger -Once -At ((Get-Date).AddMinutes(1)) -RepetitionInterval ([TimeSpan]::FromHours($hours))
if ($env:OCULUS_SCHEDULER_TEST_NAME) { $trigger = New-ScheduledTaskTrigger -Once -At ((Get-Date).AddDays(30)) -RepetitionInterval ([TimeSpan]::FromHours($hours)) }
$principal = New-ScheduledTaskPrincipal -UserId $identity.Name -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -Hidden -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::FromMinutes(10)) -MultipleInstances IgnoreNew
$description = @{cli = $cli; hours = $hours} | ConvertTo-Json -Compress
Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description $description -Force | Out-Null
"#, Some(cli), hours, test_name)?;
        Ok(())
    }

    pub fn remove() -> Result<(), String> {
        remove_named(None)
    }

    fn remove_named(test_name: Option<&str>) -> Result<(), String> {
        run(r#"
if (Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) {
    Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
}
exit 0
"#, None, 0, test_name)?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) { let _ = remove_named(Some(&self.0)); }
        }

        #[test]
        fn windows_scheduler_registers_reads_updates_and_removes_an_isolated_task() {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let name = format!("com.tchan.oculus.test-{}-{stamp}", std::process::id());
            // Cleanup runs even if an assertion panics. The test action is
            // `exit 0`, scheduled 30 days away; it never runs auth tick.
            let _cleanup = Cleanup(name.clone());
            let cli = Path::new(r"C:\Oculus test & apostrophe's\oculus.exe");
            assert!(query_named(Some(&name)).unwrap().is_none());
            install_named(cli, 6, Some(&name)).unwrap();
            let state = query_named(Some(&name)).unwrap().expect("registered task");
            assert!(state.enabled);
            assert_eq!(state.hours, 6);
            assert_eq!(state.cli, cli.to_string_lossy());
            install_named(cli, 12, Some(&name)).unwrap();
            assert_eq!(query_named(Some(&name)).unwrap().unwrap().hours, 12);
            remove_named(Some(&name)).unwrap();
            assert!(query_named(Some(&name)).unwrap().is_none());
        }
    }
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
        let dir = std::env::temp_dir().join("oculus-plist-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.plist");
        std::fs::write(&p, plist_body("/tmp/oculus", 6 * 3600)).unwrap();
        assert_eq!(interval_from_plist(&p), 6);
        // A malformed plist must fall back, not panic.
        std::fs::write(&p, "not a plist").unwrap();
        assert_eq!(interval_from_plist(&p), DEFAULT_INTERVAL_HOURS);
    }

    #[test]
    fn the_program_path_round_trips_through_the_plist() {
        let dir = std::env::temp_dir().join("oculus-plist-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("prog.plist");
        std::fs::write(&p, plist_body("/Applications/Oculus.app/Contents/MacOS/oculus", 3600))
            .unwrap();
        assert_eq!(
            program_from_plist(&p).as_deref(),
            Some("/Applications/Oculus.app/Contents/MacOS/oculus")
        );
        // A plist launchd wrote differently, or a corrupt one, must not panic.
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

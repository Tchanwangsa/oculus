//! Talking to `launchctl`: the user's launchd domain, one job's state, and a
//! `bootout` that waits for launchd to let go.

use super::LABEL;

pub(super) fn gui_domain() -> String {
    format!("gui/{}", unsafe { libc::getuid() })
}

pub(super) fn service(label: &str) -> String {
    format!("{}/{label}", gui_domain())
}

pub(super) fn launchctl(args: &[&str]) -> Result<std::process::Output, String> {
    let out = std::process::Command::new("/bin/launchctl")
        .args(args)
        .output()
        .map_err(|e| format!("launchctl: {e}"))?;
    if out.status.success() {
        return Ok(out);
    }
    let mut msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if msg.is_empty() {
        msg = format!("exit {}", out.status);
    }
    Err(msg)
}

pub(super) fn is_loaded(label: &str) -> bool {
    launchctl(&["print", &service(label)]).is_ok()
}

/// `bootout` returns before launchd has let go of the job, and a `bootstrap`
/// that comes too soon fails and leaves it unloaded. So wait for `print` to
/// stop finding the label.
pub(super) fn bootout() -> Result<(), String> {
    bootout_label(LABEL)
}

pub(super) fn bootout_label(label: &str) -> Result<(), String> {
    if !is_loaded(label) {
        return Ok(());
    }
    // An error here is usually "not loaded" racing the check above; the wait decides.
    let _ = launchctl(&["bootout", &service(label)]);
    let started = std::time::Instant::now();
    while is_loaded(label) {
        if started.elapsed() > std::time::Duration::from_secs(30) {
            return Err(format!(
                "launchd still has {label} loaded 30 s after bootout"
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(())
}

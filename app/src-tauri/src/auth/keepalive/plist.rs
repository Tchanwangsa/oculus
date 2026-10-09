//! The LaunchAgent plist: where it lives, what it says, and reading it back.

use super::{DEFAULT_INTERVAL_HOURS, LABEL};

/// The `oculus` CLI the agent runs: a sibling of the app executable (Tauri's
/// bundler copies every cargo bin into `Contents/MacOS/`), else the release build.
pub(super) fn cli_path() -> Result<std::path::PathBuf, String> {
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

pub(super) fn plist_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::PathBuf::from(home)
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist")),
    )
}

pub(super) fn plist_body(cli: &str, interval_secs: u32) -> String {
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

/// The first `<string>` inside `ProgramArguments`. Scanned, not parsed.
pub(super) fn program_from_plist(path: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let after = text.split("<key>ProgramArguments</key>").nth(1)?;
    let open = after.find("<string>")?;
    let rest = &after[open + "<string>".len()..];
    let close = rest.find("</string>")?;
    Some(rest[..close].trim().to_string())
}

pub(super) fn interval_from_plist(path: &std::path::Path) -> u32 {
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

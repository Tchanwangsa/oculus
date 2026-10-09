use std::path::PathBuf;

/// Where the LaunchAgent keep-alive logs; shown in Settings → Canvas.
pub fn keepalive_log_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("session-keepalive.log")
}

/// Append one timestamped line to the keep-alive log.
pub fn append_keepalive_log(data_dir: &std::path::Path, message: &str) {
    append_bounded_log(&keepalive_log_path(data_dir), message);
}

/// Every headless Okta sign-in attempt, whoever made it (`okta::sign_in`).
pub fn sign_in_log_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("okta-sign-in.log")
}

/// The attempt record every process checks before an automatic sign-in.
pub fn sign_in_record_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("canvas-session").join("sign-in.json")
}

pub fn append_sign_in_log(data_dir: &std::path::Path, message: &str) {
    append_bounded_log(&sign_in_log_path(data_dir), message);
}

/// Append one timestamped line, keeping the file bounded.
fn append_bounded_log(path: &std::path::Path, message: &str) {
    use std::io::Write;

    let stamp = crate::runtime::clock::now_secs();
    let line = format!("{}Z {message}\n", iso8601_utc(stamp));

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        f.write_all(line.as_bytes()).ok();
    }

    // Only rewrite once the file has grown past the cap.
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > 64 * 1024 {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let lines: Vec<&str> = text.lines().collect();
                let keep = lines[lines.len().saturating_sub(200)..].join("\n");
                std::fs::write(&path, format!("{keep}\n")).ok();
            }
        }
    }
}

/// `YYYY-MM-DDTHH:MM:SS` from a Unix timestamp, without a date crate.
pub fn iso8601_utc(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let (y, m, d) = crate::runtime::clock::civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}")
}

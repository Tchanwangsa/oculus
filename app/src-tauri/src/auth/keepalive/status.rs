//! What the settings UI shows about the agent.

#[derive(serde::Serialize)]
pub struct KeepaliveStatus {
    /// False on platforms with no LaunchAgent support.
    pub supported: bool,
    pub enabled: bool,
    pub interval_hours: u32,
    /// Last line the agent logged, so the UI can show it is actually running.
    pub last_run: Option<String>,
}

pub(super) fn last_log_line() -> Option<String> {
    let text = std::fs::read_to_string(crate::library::paths::keepalive_log_path(
        &crate::library::paths::data_dir(),
    ))
    .ok()?;
    text.lines().last().map(str::to_string)
}

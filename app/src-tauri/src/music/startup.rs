//! The one connect at startup: when Settings → Music is on (the `music`
//! settings row, `{"enabled": true}`), a session starts from the stored
//! credentials, as `music_connect` would. Off, absent or unreadable, nothing
//! connects until the user asks.

use serde::Deserialize;
use tauri::AppHandle;

use super::session::{self, Start};

const SETTING: &str = "music";

#[derive(Deserialize)]
struct Prefs {
    #[serde(default)]
    enabled: bool,
}

/// Whether the `music` row turns Spotify on; anything but a valid
/// `{"enabled": true}` is off.
pub(super) fn enabled(raw: Option<&str>) -> bool {
    raw.and_then(|text| serde_json::from_str::<Prefs>(text).ok())
        .is_some_and(|prefs| prefs.enabled)
}

/// Blocks on a database read, so callers run it off the main thread, after
/// keyd is installed: the stored credentials are keyd's to hand back.
pub fn connect_if_enabled(app: &AppHandle) {
    if enabled(crate::db::store::setting_blocking(SETTING).as_deref()) {
        session::start(app, Start::Stored);
    }
}

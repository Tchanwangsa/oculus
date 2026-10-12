//! The `music` settings row at startup, `{"enabled": bool, "output":
//! string|null}`: the output choice is applied whatever the switch says, and
//! when Settings → Music is on a session starts from the stored credentials,
//! as `music_connect` would. Off, absent or unreadable, nothing connects
//! until the user asks.

use serde::Deserialize;
use tauri::AppHandle;

use super::session::{self, Start};
use super::state;

const SETTING: &str = "music";

#[derive(Deserialize, Default, Debug, PartialEq)]
pub(super) struct Prefs {
    #[serde(default)]
    pub(super) enabled: bool,
    /// An output device's name; none is the system default.
    #[serde(default)]
    pub(super) output: Option<String>,
}

/// The row's prefs; anything unreadable is the defaults (off, the system
/// default output). Unknown keys are ignored.
pub(super) fn parse(raw: Option<&str>) -> Prefs {
    raw.and_then(|text| serde_json::from_str::<Prefs>(text).ok())
        .unwrap_or_default()
}

/// Blocks on a database read, so callers run it off the main thread, after
/// keyd is installed: the stored credentials are keyd's to hand back.
pub fn apply_settings(app: &AppHandle) {
    let prefs = parse(crate::db::store::setting_blocking(SETTING).as_deref());
    state::set_output(app, prefs.output);
    if prefs.enabled {
        session::start(app, Start::Stored);
    }
}

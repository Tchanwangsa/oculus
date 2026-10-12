//! What Oculus last played, so a dropped connection or a quit does not lose
//! it: `Memory` follows the snapshot while Oculus is the device playing, and
//! the `music_last_played` settings row keeps it across launches, written off
//! the runtime by `Saver`. A plain settings row, not a table: it needs no
//! migration. The row names its account and is ignored for any other.

mod memory;
mod saver;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use super::state::Track;

pub(super) use memory::Memory;
pub(super) use saver::Saver;

/// The settings row's key.
pub(super) const SETTING: &str = "music_last_played";

/// The row: a track in its context, where it stood, and the modes it played
/// in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct LastPlayed {
    /// The account's canonical username.
    pub account: String,
    pub context_uri: Option<String>,
    pub track: Track,
    pub position_ms: u32,
    /// Epoch ms of the write.
    #[serde(default)]
    pub saved_at_ms: u64,
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub repeat_context: bool,
    #[serde(default)]
    pub repeat_track: bool,
}

/// The row for `account`; none when absent, unreadable, another account's,
/// or naming no Spotify item. A position past the track's end is its end.
pub(super) fn parse(raw: Option<&str>, account: &str) -> Option<LastPlayed> {
    let mut row = serde_json::from_str::<LastPlayed>(raw?).ok()?;
    if row.account.is_empty() || row.account != account || !row.track.uri.starts_with("spotify:") {
        return None;
    }
    if row.track.duration_ms > 0 {
        row.position_ms = row.position_ms.min(row.track.duration_ms);
    }
    row.context_uri = row.context_uri.filter(|c| c.starts_with("spotify:"));
    Some(row)
}

pub(super) fn encode(row: &LastPlayed) -> String {
    serde_json::to_string(row).unwrap_or_default()
}

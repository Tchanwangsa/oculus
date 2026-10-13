//! The tracks Oculus itself played, for the library's Recently played
//! (`library/recently_played/`), which merges them into Spotify's own list:
//! librespot may never report a play to Spotify. `Recorder` follows the
//! snapshot and notes a track when Oculus starts playing it; the
//! `music_recent_tracks` settings row keeps them, newest first, written off
//! the runtime. A plain settings row like `music_last_played`: it names its
//! account and is ignored for any other, and sign-out leaves it for that
//! account.

mod recorder;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub(super) use recorder::Recorder;

/// The settings row's key.
pub(super) const SETTING: &str = "music_recent_tracks";
/// Tracks kept, as many as Recently played shows.
pub(in crate::music) const CAP: usize = 50;

/// A track and when it was played.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Played {
    pub uri: String,
    /// Epoch ms.
    pub played_at_ms: u64,
}

/// The row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct History {
    /// The account's canonical username.
    pub account: String,
    /// Newest first.
    #[serde(default)]
    pub items: Vec<Played>,
}

/// A `spotify:track:<id>` URI: 22 base62 characters after the prefix.
pub(in crate::music) fn is_track(uri: &str) -> bool {
    uri.strip_prefix("spotify:track:")
        .is_some_and(|id| id.len() == 22 && id.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// The row's tracks for `account`, newest first; none when the row is
/// absent, unreadable or another account's. Anything not a track URI is
/// dropped.
pub(in crate::music) fn parse(raw: Option<&str>, account: &str) -> Vec<Played> {
    let Some(row) = raw.and_then(|r| serde_json::from_str::<History>(r).ok()) else {
        return Vec::new();
    };
    if row.account.is_empty() || row.account != account {
        return Vec::new();
    }
    newest_first(row.items.into_iter().filter(|p| is_track(&p.uri)))
}

pub(super) fn encode(row: &History) -> String {
    serde_json::to_string(row).unwrap_or_default()
}

/// `items` with `played` noted.
pub(super) fn record(items: Vec<Played>, played: Played) -> Vec<Played> {
    newest_first(std::iter::once(played).chain(items))
}

/// One entry per URI at its newest time, newest first, up to `CAP`. Equal
/// times keep the order they came in.
fn newest_first(items: impl IntoIterator<Item = Played>) -> Vec<Played> {
    let mut out: Vec<Played> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    for played in items {
        match at.get(&played.uri) {
            Some(&i) => out[i].played_at_ms = out[i].played_at_ms.max(played.played_at_ms),
            None => {
                at.insert(played.uri.clone(), out.len());
                out.push(played);
            }
        }
    }
    out.sort_by_key(|p| std::cmp::Reverse(p.played_at_ms));
    out.truncate(CAP);
    out
}

/// The row for `account`, read on a thread of its own.
pub(in crate::music) fn read_blocking(account: &str) -> Vec<Played> {
    parse(
        crate::db::store::setting_blocking(SETTING).as_deref(),
        account,
    )
}

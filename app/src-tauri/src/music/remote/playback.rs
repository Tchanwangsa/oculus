//! What another Connect device is playing, from the pushed cluster's player
//! state, for the snapshot's `remote`. The track's name, artists and cover
//! come first from the player state's track metadata when it carries them,
//! then from the library's track row (`library::track_rows`), fetched once
//! per URI and kept like any row.

use librespot_protocol::connect::Cluster as ClusterProto;
use librespot_protocol::player::{PlayerState, ProvidedTrack};
use serde::Serialize;
use tauri::AppHandle;

use crate::music::library::{self, TrackRow};
use crate::music::state::{self, now_ms, percent, Track};

const IMAGE_BASE: &str = "https://i.scdn.co/image/";

/// The playback of the active device while it is another one.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RemotePlayback {
    pub device_name: String,
    /// Unknown until the player state's metadata or the library names it.
    pub track: Option<Track>,
    /// The position at `position_at_ms` (epoch ms), as the local fields.
    pub position_ms: u32,
    pub position_at_ms: u64,
    pub playing: bool,
    /// 0–100; none when the device takes no volume commands.
    pub volume: Option<u8>,
    pub shuffle: bool,
    pub repeat: bool,
    /// Its previous goes back a track: its player state's restrictions name
    /// no reason against it (a phone at its first track says `no_prev_track`).
    pub can_prev: bool,
    /// The playing item's URI, for matching a row fetched later.
    #[serde(skip)]
    pub(in crate::music) uri: String,
}

/// The other device's playback, or `None` when nothing is active, Oculus
/// is, or the push carried no player state.
pub(in crate::music) fn from_cluster(
    cluster: &ClusterProto,
    own_id: &str,
) -> Option<RemotePlayback> {
    let active = cluster.active_device_id.as_str();
    if active.is_empty() || active == own_id {
        return None;
    }
    let player = cluster.player_state.as_ref()?;
    let device = cluster.device.get(active);
    let options = player.options.as_ref();
    let uri = player
        .track
        .as_ref()
        .map(|t| t.uri.clone())
        .unwrap_or_default();
    Some(RemotePlayback {
        device_name: device.map(|d| d.name.clone()).unwrap_or_default(),
        track: player
            .track
            .as_ref()
            .and_then(|t| from_metadata(t, duration(player))),
        position_ms: u32::try_from(player.position_as_of_timestamp.max(0)).unwrap_or(u32::MAX),
        position_at_ms: u64::try_from(player.timestamp)
            .ok()
            .filter(|t| *t > 0)
            .unwrap_or_else(now_ms),
        playing: player.is_playing && !player.is_paused,
        volume: device
            .filter(|d| !d.capabilities.as_ref().is_some_and(|c| c.disable_volume))
            .map(|d| percent(d.volume.min(u16::MAX as u32) as u16)),
        shuffle: options.is_some_and(|o| o.shuffling_context),
        repeat: options.is_some_and(|o| o.repeating_context || o.repeating_track),
        can_prev: player
            .restrictions
            .as_ref()
            .is_none_or(|r| r.disallow_skipping_prev_reasons.is_empty()),
        uri,
    })
}

fn duration(player: &PlayerState) -> u32 {
    u32::try_from(player.duration.max(0)).unwrap_or(u32::MAX)
}

/// A track from the metadata Spotify's apps attach to the track they play
/// (`title`, `album_title`, `artist_name`, `image_large_url` or
/// `image_url`), when it names a title; librespot sends none of these.
fn from_metadata(track: &ProvidedTrack, duration_ms: u32) -> Option<Track> {
    let meta = |key: &str| track.metadata.get(key).filter(|v| !v.is_empty());
    let name = meta("title")?;
    Some(Track {
        uri: track.uri.clone(),
        name: name.clone(),
        artists: meta("artist_name").into_iter().cloned().collect(),
        album: meta("album_title").cloned().unwrap_or_default(),
        cover_url: meta("image_large_url")
            .or(meta("image_url"))
            .and_then(|v| image_url(v)),
        duration_ms,
    })
}

/// `spotify:image:<hex>` or an image URL as Spotify's CDN URL.
fn image_url(value: &str) -> Option<String> {
    let id = value.rsplit([':', '/']).next()?;
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("{IMAGE_BASE}{id}"))
}

/// The library's row for the playing track over what the metadata said; a
/// cover or a duration the row lacks keeps the metadata's.
pub(in crate::music) fn with_row(remote: &mut RemotePlayback, row: &TrackRow) {
    let before = remote.track.take();
    let duration_ms = before
        .as_ref()
        .map(|t| t.duration_ms)
        .filter(|d| *d > 0)
        .unwrap_or(row.duration_ms);
    remote.track = Some(Track {
        uri: row.uri.clone(),
        name: row.name.clone(),
        artists: row.artists.clone(),
        album: row.album.clone(),
        cover_url: before
            .and_then(|t| t.cover_url)
            .or_else(|| row.image_url.clone()),
        duration_ms,
    });
}

/// Fills `remote`'s track from a row the library keeps; without one, asks
/// for it once per URI (`asked` holds the last URI asked) and lays it on
/// run `id`'s snapshot when it comes. A row Spotify had nothing for leaves
/// the metadata's track.
pub(in crate::music) fn describe(
    app: &AppHandle,
    id: u64,
    account: &str,
    remote: &mut RemotePlayback,
    asked: &mut Option<String>,
) {
    if !remote.uri.starts_with("spotify:track:") {
        return;
    }
    match library::kept_row(app, account, &remote.uri) {
        Some(Some(row)) => with_row(remote, &row),
        Some(None) => {}
        None if asked.as_deref() == Some(remote.uri.as_str()) => {}
        None => {
            *asked = Some(remote.uri.clone());
            let app = app.clone();
            let uri = remote.uri.clone();
            tauri::async_runtime::spawn(async move {
                let rows = library::track_rows(&app, std::slice::from_ref(&uri)).await;
                if let Some(row) = rows.ok().and_then(|mut rows| rows.remove(&uri)) {
                    state::set_remote_row(&app, id, &row);
                }
            });
        }
    }
}

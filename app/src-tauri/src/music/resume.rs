//! Loading what Oculus last played back onto this device: after a reconnect,
//! when Oculus was the device playing at the drop, and from Play when nothing
//! plays anywhere (`control.rs`). The track is loaded in its context at its
//! position with its modes, playing or paused. Queued tracks are not kept:
//! the load starts the context afresh.

use std::time::{Duration, Instant};

use librespot_connect::{
    LoadContextOptions, LoadRequest, LoadRequestOptions, Options, PlayingTrack,
};
use tauri::AppHandle;

use super::control;
use super::last_played::{self, LastPlayed};
use super::state;
use crate::runtime::blocking;

/// How long a resume waits for the first Connect cluster push after a
/// reconnect, to see whether another device took playback meanwhile.
const CLUSTER_WAIT: Duration = Duration::from_secs(2);

/// The load for `row`: its context at its track, or the track alone when the
/// context is none or is the track itself.
pub(super) fn request(row: &LastPlayed, play: bool) -> LoadRequest {
    let options = |playing_track| LoadRequestOptions {
        start_playing: play,
        seek_to: row.position_ms,
        playing_track,
        context_options: Some(LoadContextOptions::Options(Options {
            shuffle: row.shuffle,
            repeat: row.repeat_context,
            repeat_track: row.repeat_track,
        })),
    };
    let uri = row.track.uri.clone();
    match row.context_uri.clone().filter(|c| *c != uri) {
        Some(context) => {
            LoadRequest::from_context_uri(context, options(Some(PlayingTrack::Uri(uri))))
        }
        None => LoadRequest::from_tracks(vec![uri], options(None)),
    }
}

/// Activates this device and loads `row`. The Spirc handles both in order,
/// so the load is not dropped as an inactive device's.
pub(super) fn restore(app: &AppHandle, row: &LastPlayed, play: bool) -> Result<(), String> {
    let load = request(row, play);
    state::with_spirc(app, |spirc| {
        spirc.activate()?;
        spirc.load(load)
    })
}

/// After run `id` connects as `account` on the session `session_id`: shows
/// the remembered row when nothing is shown, then resumes what the drop
/// interrupted unless another device took playback meanwhile. A session
/// that drops meanwhile leaves the resume to the next one.
pub(super) async fn after_connect(app: AppHandle, id: u64, account: String, session_id: String) {
    recall(&app, id, &account).await;
    if !state::resume_pending(&app, id) {
        return;
    }
    let waited = Instant::now();
    while state::cluster(&app).is_none() && waited.elapsed() < CLUSTER_WAIT {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let current = state::session(&app).is_some_and(|s| s.session_id() == session_id);
    if !current {
        return;
    }
    let Some((row, play)) = state::take_resume(&app, id) else {
        return;
    };
    if state::active(&app) != Some(false) {
        return;
    }
    if let Some(device) = control::other_device(&app) {
        elog!(
            "[music] {} took playback while reconnecting; not resuming here",
            device.name
        );
        return;
    }
    if let Err(why) = restore(&app, &row, play) {
        elog!("[music] could not resume after reconnecting: {why}");
        state::update_run(&app, id, |s| {
            s.error = Some(format!("Could not resume playback: {why}"));
        });
    }
}

/// Reads the row and shows it, when run `id` has nothing to show.
pub(super) async fn recall(app: &AppHandle, id: u64, account: &str) {
    if !state::needs_recall(app, id, account) {
        return;
    }
    let raw = blocking::run(|| Ok(crate::db::store::setting_blocking(last_played::SETTING)))
        .await
        .ok()
        .flatten();
    if let Some(row) = last_played::parse(raw.as_deref(), account) {
        state::recall(app, id, row);
    }
}

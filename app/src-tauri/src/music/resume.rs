//! Loading what Oculus last played back onto this device: after a reconnect,
//! when Oculus was the device playing at the drop, and from Play when nothing
//! plays anywhere (`control.rs`). The track is loaded in its context at its
//! position with its modes, playing or paused. Queued tracks are not kept:
//! the load starts the context afresh.

use std::time::Duration;

use librespot_connect::{
    LoadContextOptions, LoadRequest, LoadRequestOptions, Options, PlayingTrack,
};
use librespot_protocol::connect::Cluster;
use tauri::AppHandle;
use tokio::sync::watch;

use super::last_played::{self, LastPlayed};
use super::state;
use crate::runtime::blocking;

/// A safety cap on waiting for the announce's answer, which normally comes
/// within a second of the connect: the wait also ends with the Spirc's task.
const ANNOUNCE_CAP: Duration = Duration::from_secs(15);

/// What the announce's answer says of a waiting resume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Decision {
    Resume,
    /// Another device is playing: playback is not taken back from it.
    Elsewhere(String),
    /// librespot takes the answer's transfer state over by itself (nothing
    /// active, transfer data, and the player state's session ours or
    /// unnamed): a resume would load over it.
    TakenOver,
}

/// The decision for a resume on the session `session_id` of the device
/// `own_id`, from the cluster Spotify answered its announce with.
pub(super) fn decide(cluster: &Cluster, own_id: &str, session_id: &str) -> Decision {
    let active = cluster.active_device_id.as_str();
    if !active.is_empty() && active != own_id {
        let name = cluster
            .device
            .get(active)
            .map(|d| d.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "another device".into());
        return Decision::Elsewhere(name);
    }
    let theirs = cluster.player_state.session_id.as_str();
    let same_session = theirs == session_id || theirs.is_empty();
    if active.is_empty() && same_session && !cluster.transfer_data.is_empty() {
        return Decision::TakenOver;
    }
    Decision::Resume
}

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
/// the remembered row when nothing is shown, then, once Spotify has answered
/// the Spirc's announce (`announced`), resumes what the drop interrupted
/// unless that answer says another device plays. A session that drops or
/// stops first leaves the resume to the next one.
pub(super) async fn after_connect(
    app: AppHandle,
    id: u64,
    account: String,
    session_id: String,
    mut announced: watch::Receiver<Option<Cluster>>,
) {
    recall(&app, id, &account).await;
    if !state::resume_pending(&app, id) {
        return;
    }
    let cluster = match tokio::time::timeout(ANNOUNCE_CAP, announced.wait_for(Option::is_some))
        .await
    {
        Ok(Ok(cluster)) => cluster.clone(),
        // The Spirc's task ended: a stop, or the session dropped.
        Ok(Err(_)) => return,
        Err(_) => {
            elog!("[music] Spotify did not answer the device's announce within 15 s; not resuming yet");
            return;
        }
    };
    let Some(cluster) = cluster else {
        return;
    };
    let Some(session) = state::session(&app).filter(|s| s.session_id() == session_id) else {
        return;
    };
    let Some((row, play)) = state::take_resume(&app, id) else {
        return;
    };
    if state::active(&app) != Some(false) {
        return;
    }
    match decide(&cluster, session.device_id(), &session_id) {
        Decision::Resume => {}
        Decision::Elsewhere(device) => {
            elog!("[music] {device} took playback while reconnecting; not resuming here");
            return;
        }
        Decision::TakenOver => {
            elog!("[music] librespot took over Spotify's transfer state; not resuming over it");
            return;
        }
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

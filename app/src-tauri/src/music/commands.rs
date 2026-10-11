//! The music commands. Each returns as soon as its work is handed off; the
//! outcome arrives as `music-state`. Playback commands go to the connected
//! device and fail with "Not connected to Spotify" otherwise; while another
//! device holds playback they pull it back first (`control.rs`). The lists
//! (`music_playlists`, `music_playlist_covers`, `music_playlist_tracks`,
//! `music_tracks`, `music_search`, `music_devices`, `music_outputs`) return
//! their results.

use std::collections::HashMap;

use librespot_connect::{LoadRequest, LoadRequestOptions, PlayingTrack, Spirc};
use librespot_core::{SessionConfig, SpotifyUri};
use tauri::AppHandle;

use super::audio::{self, OutputDevice};
use super::control::{self, Need};
use super::devices::{self, Device};
use super::library::{self, Playlist, SearchResults, TrackRow};
use super::login::{self, LoginError};
use super::session::{self, Start};
use super::state::{self, Snapshot};
use crate::runtime::blocking;

/// For the frontend's mount, which may have missed earlier events.
#[tauri::command]
pub fn music_state(app: AppHandle) -> Snapshot {
    state::snapshot(&app)
}

/// Opens Spotify's login in the browser and connects once it is approved.
/// A session already running is stopped first; a login already waiting is
/// replaced, so asking again just opens the page again.
#[tauri::command]
pub fn music_login(app: AppHandle) {
    state::stop_run(&app);
    let cancel = state::begin_login(&app);
    state::update(&app, |s| {
        s.connecting = true;
        s.needs_login = false;
        s.error = None;
    });
    std::thread::spawn(move || {
        let client_id = SessionConfig::default().client_id;
        let result = login::authorize(&client_id, &cancel, |url| {
            if let Err(e) = tauri_plugin_opener::open_url(url, None::<&str>) {
                elog!("[music] could not open the Spotify login: {e}");
            }
        });
        if !state::end_login(&app, &cancel) {
            return;
        }
        match result {
            // A session `music_connect` started meanwhile is left running.
            Ok(token) => {
                session::start(&app, Start::Token(token));
            }
            Err(LoginError::Cancelled) => state::update(&app, |s| s.connecting = false),
            Err(e) => {
                elog!("[music] {e}");
                state::update(&app, |s| {
                    s.connecting = false;
                    s.error = Some(e.to_string());
                });
            }
        }
    });
}

/// Connects from the stored credentials; `needs_login` when there are none.
#[tauri::command]
pub fn music_connect(app: AppHandle) {
    session::start(&app, Start::Stored);
}

/// Leaves Spotify, keeping the stored credentials.
#[tauri::command]
pub fn music_disconnect(app: AppHandle) {
    state::cancel_login(&app);
    state::stop_run(&app);
    state::update(&app, |s| s.set_idle());
}

/// Leaves Spotify and deletes the stored credentials from keyd.
#[tauri::command]
pub async fn music_sign_out(app: AppHandle) -> Result<(), String> {
    music_disconnect(app.clone());
    let deleted = blocking::run(super::credentials::delete).await;
    state::update(&app, |s| {
        s.account = None;
        s.track = None;
        s.needs_login = true;
        s.error = deleted.as_ref().err().cloned();
    });
    deleted
}

/// Runs `f` once this device is active, pulling playback here first.
async fn pulled(
    app: &AppHandle,
    f: impl FnOnce(&Spirc) -> Result<(), librespot_core::Error>,
) -> Result<(), String> {
    control::send(app, Need::Pull, || state::with_spirc(app, f)).await
}

/// While another device plays, a toggle would pause what the pull brings
/// over, so it pulls and plays.
#[tauri::command]
pub async fn music_play_pause(app: AppHandle) -> Result<(), String> {
    if state::active(&app).ok_or("Not connected to Spotify")? {
        state::with_spirc(&app, |spirc| spirc.play_pause())
    } else {
        pulled(&app, |spirc| spirc.play()).await
    }
}

/// Explicit, unlike the toggle: what pauses music when a lecture starts
/// (`app/src/lib/music/exclusive/`) must never start it instead.
#[tauri::command]
pub async fn music_play(app: AppHandle) -> Result<(), String> {
    pulled(&app, |spirc| spirc.play()).await
}

/// Nothing while another device plays: a pause never pulls playback here.
#[tauri::command]
pub async fn music_pause(app: AppHandle) -> Result<(), String> {
    control::send(&app, Need::IfActive, || {
        state::with_spirc(&app, |spirc| spirc.pause())
    })
    .await
}

#[tauri::command]
pub async fn music_next(app: AppHandle) -> Result<(), String> {
    pulled(&app, |spirc| spirc.next()).await
}

#[tauri::command]
pub async fn music_prev(app: AppHandle) -> Result<(), String> {
    pulled(&app, |spirc| spirc.prev()).await
}

#[tauri::command]
pub async fn music_seek(app: AppHandle, position_ms: u32) -> Result<(), String> {
    pulled(&app, |spirc| spirc.set_position_ms(position_ms)).await
}

/// `percent` is 0–100; more is taken as 100. Drops a held duck.
#[tauri::command]
pub async fn music_volume(app: AppHandle, percent: u8) -> Result<(), String> {
    control::send(&app, Need::Pull, || state::set_volume(&app, percent)).await
}

/// The seam a focus mode calls. `Some(level)` lowers the volume to `level`%
/// of what it was before the first duck; `None` puts that volume back. A
/// volume the user sets in between wins, and the unduck then does nothing.
/// Nothing while another device plays.
#[tauri::command]
pub async fn music_duck(app: AppHandle, level: Option<u8>) -> Result<(), String> {
    control::send(&app, Need::IfActive, || state::duck(&app, level)).await
}

#[tauri::command]
pub async fn music_shuffle(app: AppHandle, on: bool) -> Result<(), String> {
    pulled(&app, |spirc| spirc.shuffle(on)).await
}

/// On repeats the context. The snapshot's `repeat` is also true for a
/// repeated track (set from the phone), so off clears both.
#[tauri::command]
pub async fn music_repeat(app: AppHandle, on: bool) -> Result<(), String> {
    pulled(&app, |spirc| {
        if !on {
            spirc.repeat_track(false)?;
        }
        spirc.repeat(on)
    })
    .await
}

/// Plays a context (`spotify:playlist:…`, `spotify:album:…`) here.
#[tauri::command]
pub async fn music_play_context(app: AppHandle, uri: String) -> Result<(), String> {
    spotify_uri(&uri)?;
    load(
        &app,
        LoadRequest::from_context_uri(
            uri,
            LoadRequestOptions {
                start_playing: true,
                ..Default::default()
            },
        ),
    )
    .await
}

/// Plays a track here: from its place in `context_uri` (its album from a
/// search, or the playlist it was picked in) so playback goes on through
/// it, or alone without one. librespot finds the track by URI among the
/// context's loaded tracks. A track URI is not a context librespot can load.
#[tauri::command]
pub async fn music_play_track(
    app: AppHandle,
    uri: String,
    context_uri: Option<String>,
) -> Result<(), String> {
    if !uri.starts_with("spotify:track:") {
        return Err(format!("Not a Spotify track: {uri}"));
    }
    let options = |playing_track| LoadRequestOptions {
        start_playing: true,
        playing_track,
        ..Default::default()
    };
    let request = match context_uri {
        Some(context) => {
            spotify_uri(&context)?;
            LoadRequest::from_context_uri(context, options(Some(PlayingTrack::Uri(uri))))
        }
        None => LoadRequest::from_tracks(vec![uri], options(None)),
    };
    load(&app, request).await
}

fn spotify_uri(uri: &str) -> Result<(), String> {
    if uri.starts_with("spotify:") {
        Ok(())
    } else {
        Err(format!("Not a Spotify URI: {uri}"))
    }
}

/// Spotify ignores a load sent to a device that is not active, so this
/// activates it first; the load replaces whatever a pull would bring over.
async fn load(app: &AppHandle, request: LoadRequest) -> Result<(), String> {
    control::send(app, Need::Local, || {
        state::with_spirc(app, |spirc| spirc.load(request))
    })
    .await?;
    state::update(app, |s| s.error = None);
    Ok(())
}

/// The account's playlists: kept in memory after the first fetch, fetched
/// again only on `refresh`.
#[tauri::command]
pub async fn music_playlists(app: AppHandle, refresh: bool) -> Result<Vec<Playlist>, String> {
    Ok(library::playlists(&app, refresh).await?)
}

/// Covers for playlists that have none, by URI: one URL, or four for a
/// grid; empty when the playlist has no track covers. Kept in memory like
/// the playlists; a URI missing from the answer can be asked for again.
#[tauri::command]
pub async fn music_playlist_covers(
    app: AppHandle,
    uris: Vec<String>,
) -> Result<HashMap<String, Vec<String>>, String> {
    Ok(library::playlist_covers(&app, uris).await?)
}

/// Every catalogue track of a playlist, in order, as URIs; kept until the
/// playlists are refreshed. Rows come from `music_tracks`.
#[tauri::command]
pub async fn music_playlist_tracks(app: AppHandle, uri: String) -> Result<Vec<String>, String> {
    Ok(library::playlist_tracks(&app, &uri).await?)
}

/// Rows for these tracks, by URI; one Spotify describes nothing for is left
/// out. Kept in memory once fetched.
#[tauri::command]
pub async fn music_tracks(
    app: AppHandle,
    uris: Vec<String>,
) -> Result<HashMap<String, TrackRow>, String> {
    Ok(library::track_rows(&app, &uris).await?)
}

/// Adds a track to the queue, only while Oculus is the active device: the
/// Spirc drops a queue command otherwise, and librespot has no way to queue
/// on another device (`control.rs`).
#[tauri::command]
pub async fn music_add_to_queue(app: AppHandle, uri: String) -> Result<(), String> {
    let track = SpotifyUri::from_uri(&uri).map_err(|_| format!("Not a Spotify URI: {uri}"))?;
    if !matches!(track, SpotifyUri::Track { .. }) {
        return Err(format!("Not a Spotify track: {uri}"));
    }
    control::send(&app, Need::Active, || {
        state::with_spirc(&app, |spirc| spirc.add_to_queue(track))
    })
    .await
}

/// Playlists, albums and tracks matching `query`, ten of each.
#[tauri::command]
pub async fn music_search(app: AppHandle, query: String) -> Result<SearchResults, String> {
    Ok(library::search(&app, &query).await?)
}

/// The account's Connect devices from the last pushed cluster; only this
/// device until the first push.
#[tauri::command]
pub fn music_devices(app: AppHandle) -> Result<Vec<Device>, String> {
    devices::list(&app)
}

/// Moves playback to `device_id` (this device's id pulls it here) and plays.
#[tauri::command]
pub async fn music_transfer(app: AppHandle, device_id: String) -> Result<(), String> {
    devices::transfer(&app, device_id).await
}

/// This Mac's output devices, the current default marked.
#[tauri::command]
pub async fn music_outputs() -> Result<Vec<OutputDevice>, String> {
    blocking::run(audio::list).await
}

/// Chooses the output (`None`: the system default). Playback moves at once;
/// the frontend keeps the choice in the `music` settings row.
#[tauri::command]
pub fn music_set_output(app: AppHandle, name: Option<String>) {
    state::set_output(&app, name);
}

//! The music commands. Each returns as soon as its work is handed off; the
//! outcome arrives as `music-state`. Playback commands need a connected
//! device and fail with "Not connected to Spotify" otherwise; while another
//! device holds playback they control it remotely, and with nothing active
//! anywhere they pull or activate playback here (`control.rs`). The lists
//! (`music_playlists`, `music_playlist_covers`, `music_playlist_tracks`,
//! `music_album_tracks`, `music_tracks`, `music_search`, `music_search_more`,
//! `music_devices`, `music_outputs`) return their results.

use std::collections::HashMap;

use librespot_connect::{LoadRequest, LoadRequestOptions, PlayingTrack, Spirc};
use librespot_core::{SessionConfig, SpotifyUri};
use tauri::AppHandle;

use super::audio::{self, OutputDevice};
use super::control::{self, Need, Route};
use super::devices::{self, Device};
use super::library::{self, Playlist, SearchKind, SearchPage, SearchResults, TrackRow};
use super::login::{self, LoginError};
use super::queue;
use super::remote::{self, RemoteCommand};
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

/// Runs `local` on this device's Spirc, or sends `remote` to the device
/// playing elsewhere, as `need` routes it (`control.rs`).
async fn command(
    app: &AppHandle,
    need: Need,
    local: impl FnOnce(&Spirc) -> Result<(), librespot_core::Error>,
    remote: RemoteCommand,
) -> Result<(), String> {
    control::send(app, need, || state::with_spirc(app, local), |_| remote).await
}

/// Here, a toggle; with nothing active, a pull that plays (a toggle would
/// pause what the pull brings over); elsewhere, the other device's pause or
/// resume by what it is doing.
#[tauri::command]
pub async fn music_play_pause(app: AppHandle) -> Result<(), String> {
    let active = state::active(&app).ok_or("Not connected to Spotify")?;
    control::send(
        &app,
        Need::Control,
        || {
            state::with_spirc(&app, |spirc| {
                if active {
                    spirc.play_pause()
                } else {
                    spirc.play()
                }
            })
        },
        |device| {
            if device.playing {
                RemoteCommand::Pause
            } else {
                RemoteCommand::Resume
            }
        },
    )
    .await
}

/// The player's play button: explicit, unlike the toggle, so a click on a
/// stale button never pauses.
#[tauri::command]
pub async fn music_play(app: AppHandle) -> Result<(), String> {
    command(
        &app,
        Need::Control,
        |spirc| spirc.play(),
        RemoteCommand::Resume,
    )
    .await
}

/// What the exclusive-playback rule (`app/src/lib/music/exclusive/`) sends:
/// pauses this device only. A lecture starting on the Mac never pauses
/// another device, and a pause never pulls playback here.
#[tauri::command]
pub async fn music_pause(app: AppHandle) -> Result<(), String> {
    control::if_active(&app, || state::with_spirc(&app, |spirc| spirc.pause())).await
}

/// The player's pause button: pauses what the player shows, here or on the
/// device playing elsewhere; nothing when nothing is active.
#[tauri::command]
pub async fn music_pause_player(app: AppHandle) -> Result<(), String> {
    command(
        &app,
        Need::NeverPull,
        |spirc| spirc.pause(),
        RemoteCommand::Pause,
    )
    .await
}

#[tauri::command]
pub async fn music_next(app: AppHandle) -> Result<(), String> {
    command(
        &app,
        Need::Control,
        |spirc| spirc.next(),
        RemoteCommand::SkipNext,
    )
    .await
}

#[tauri::command]
pub async fn music_prev(app: AppHandle) -> Result<(), String> {
    command(
        &app,
        Need::Control,
        |spirc| spirc.prev(),
        RemoteCommand::SkipPrev,
    )
    .await
}

#[tauri::command]
pub async fn music_seek(app: AppHandle, position_ms: u32) -> Result<(), String> {
    command(
        &app,
        Need::Control,
        |spirc| spirc.set_position_ms(position_ms),
        RemoteCommand::SeekTo(position_ms),
    )
    .await
}

/// `percent` is 0–100; more is taken as 100. Here it drops a held duck.
#[tauri::command]
pub async fn music_volume(app: AppHandle, percent: u8) -> Result<(), String> {
    let percent = percent.min(100);
    control::send(
        &app,
        Need::Control,
        || state::set_volume(&app, percent),
        |_| RemoteCommand::Volume(percent),
    )
    .await
}

/// The seam a focus mode calls. `Some(level)` lowers the volume to `level`%
/// of what it was before the first duck; `None` puts that volume back. A
/// volume the user sets in between wins, and the unduck then does nothing.
/// This device only: nothing while another device plays.
#[tauri::command]
pub async fn music_duck(app: AppHandle, level: Option<u8>) -> Result<(), String> {
    control::if_active(&app, || state::duck(&app, level)).await
}

#[tauri::command]
pub async fn music_shuffle(app: AppHandle, on: bool) -> Result<(), String> {
    command(
        &app,
        Need::Control,
        |spirc| spirc.shuffle(on),
        RemoteCommand::Shuffle(on),
    )
    .await
}

/// On repeats the context. The snapshot's `repeat` is also true for a
/// repeated track (set from the phone), so off clears both.
#[tauri::command]
pub async fn music_repeat(app: AppHandle, on: bool) -> Result<(), String> {
    let route = control::route(&app, Need::Control).await?;
    match route {
        Route::Here => state::with_spirc(&app, |spirc| {
            if !on {
                spirc.repeat_track(false)?;
            }
            spirc.repeat(on)
        }),
        Route::There(device) => {
            if !on {
                remote::send(&app, &device, RemoteCommand::RepeatTrack(false)).await?;
            }
            remote::send(&app, &device, RemoteCommand::RepeatContext(on)).await
        }
        Route::Skip => Ok(()),
    }
}

/// Plays a context (`spotify:playlist:…`, `spotify:album:…`): on the device
/// playing elsewhere, else here.
#[tauri::command]
pub async fn music_play_context(app: AppHandle, uri: String) -> Result<(), String> {
    spotify_uri(&uri)?;
    let remote = RemoteCommand::Play {
        context: uri.clone(),
        track: None,
    };
    load(
        &app,
        LoadRequest::from_context_uri(
            uri,
            LoadRequestOptions {
                start_playing: true,
                ..Default::default()
            },
        ),
        remote,
    )
    .await
}

/// Plays a track: from its place in `context_uri` (its album from a search,
/// or the playlist it was picked in) so playback goes on through it, or
/// alone without one. librespot finds the track by URI among the context's
/// loaded tracks. A track URI is not a context librespot can load; another
/// device is sent the track as its own context.
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
    let (request, remote) = match context_uri {
        Some(context) => {
            spotify_uri(&context)?;
            let remote = RemoteCommand::Play {
                context: context.clone(),
                track: Some(uri.clone()),
            };
            let request =
                LoadRequest::from_context_uri(context, options(Some(PlayingTrack::Uri(uri))));
            (request, remote)
        }
        None => {
            let remote = RemoteCommand::Play {
                context: uri.clone(),
                track: None,
            };
            (LoadRequest::from_tracks(vec![uri], options(None)), remote)
        }
    };
    load(&app, request, remote).await
}

fn spotify_uri(uri: &str) -> Result<(), String> {
    if uri.starts_with("spotify:") {
        Ok(())
    } else {
        Err(format!("Not a Spotify URI: {uri}"))
    }
}

/// Spotify ignores a load sent to a device that is not active, so with
/// nothing active this activates it first; the load replaces whatever a pull
/// would bring over. Another device playing is sent `remote` instead.
async fn load(app: &AppHandle, request: LoadRequest, remote: RemoteCommand) -> Result<(), String> {
    command(app, Need::Load, |spirc| spirc.load(request), remote).await?;
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

/// Every track of an album, in order, as URIs. Rows come from
/// `music_tracks`.
#[tauri::command]
pub async fn music_album_tracks(app: AppHandle, uri: String) -> Result<Vec<String>, String> {
    if !uri.starts_with("spotify:album:") {
        return Err(format!("Not a Spotify album: {uri}"));
    }
    Ok(library::album_tracks(&app, &uri).await?)
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

/// Adds a track to the queue of the device playing: Oculus's own, or
/// another device's by remote; refused with nothing active (`control.rs`).
#[tauri::command]
pub async fn music_add_to_queue(app: AppHandle, uri: String) -> Result<(), String> {
    let track = SpotifyUri::from_uri(&uri).map_err(|_| format!("Not a Spotify URI: {uri}"))?;
    if !matches!(track, SpotifyUri::Track { .. }) {
        return Err(format!("Not a Spotify track: {uri}"));
    }
    command(
        &app,
        Need::Queue,
        |spirc| spirc.add_to_queue(track),
        RemoteCommand::AddToQueue(uri),
    )
    .await
}

/// Plays the snapshot's `up_next` entry at `index`, named `uri`, on the
/// device playing, Oculus or another, skipping what comes before it
/// (`queue.rs`).
#[tauri::command]
pub async fn music_skip_to(app: AppHandle, uri: String, index: usize) -> Result<(), String> {
    queue::skip_to(&app, &uri, index).await
}

/// Takes the snapshot's `up_next` entry at `index`, named `uri`, out of the
/// queue of the device playing (`queue.rs`).
#[tauri::command]
pub async fn music_queue_remove(app: AppHandle, uri: String, index: usize) -> Result<(), String> {
    queue::remove(&app, &uri, index).await
}

/// Tracks, artists, albums and playlists matching `query`, ten of each.
#[tauri::command]
pub async fn music_search(app: AppHandle, query: String) -> Result<SearchResults, String> {
    Ok(library::search(&app, &query).await?)
}

/// The page of `kind` matching `query` from `offset` on, for a list that
/// keeps loading as it scrolls.
#[tauri::command]
pub async fn music_search_more(
    app: AppHandle,
    query: String,
    kind: SearchKind,
    offset: u32,
) -> Result<SearchPage, String> {
    Ok(library::search_page(&app, &query, kind, offset).await?)
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

/// "Play here": moves playback from the device playing elsewhere to this
/// one and plays.
#[tauri::command]
pub async fn music_play_here(app: AppHandle) -> Result<(), String> {
    control::play_here(&app).await
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

//! The music commands. Each returns as soon as its work is handed off; the
//! outcome arrives as `music-state`. Playback commands go to the connected
//! device and fail with "Not connected to Spotify" otherwise.

use librespot_connect::{LoadRequest, LoadRequestOptions};
use librespot_core::SessionConfig;
use tauri::AppHandle;

use super::login::{self, LoginError};
use super::session::{self, Start};
use super::state::{self, from_percent, Snapshot};
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
                eprintln!("[music] could not open the Spotify login: {e}");
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
                eprintln!("[music] {e}");
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

#[tauri::command]
pub fn music_play_pause(app: AppHandle) -> Result<(), String> {
    state::with_spirc(&app, |spirc| spirc.play_pause())
}

/// Explicit, unlike the toggle: what pauses music when a lecture starts
/// (`app/src/lib/music/exclusive.ts`) must never start it instead.
#[tauri::command]
pub fn music_play(app: AppHandle) -> Result<(), String> {
    state::with_spirc(&app, |spirc| spirc.play())
}

#[tauri::command]
pub fn music_pause(app: AppHandle) -> Result<(), String> {
    state::with_spirc(&app, |spirc| spirc.pause())
}

#[tauri::command]
pub fn music_next(app: AppHandle) -> Result<(), String> {
    state::with_spirc(&app, |spirc| spirc.next())
}

#[tauri::command]
pub fn music_prev(app: AppHandle) -> Result<(), String> {
    state::with_spirc(&app, |spirc| spirc.prev())
}

#[tauri::command]
pub fn music_seek(app: AppHandle, position_ms: u32) -> Result<(), String> {
    state::with_spirc(&app, |spirc| spirc.set_position_ms(position_ms))
}

/// `percent` is 0–100; more is taken as 100.
#[tauri::command]
pub fn music_volume(app: AppHandle, percent: u8) -> Result<(), String> {
    state::with_spirc(&app, |spirc| spirc.set_volume(from_percent(percent)))
}

#[tauri::command]
pub fn music_shuffle(app: AppHandle, on: bool) -> Result<(), String> {
    state::with_spirc(&app, |spirc| spirc.shuffle(on))
}

/// On repeats the context. The snapshot's `repeat` is also true for a
/// repeated track (set from the phone), so off clears both.
#[tauri::command]
pub fn music_repeat(app: AppHandle, on: bool) -> Result<(), String> {
    state::with_spirc(&app, |spirc| {
        if !on {
            spirc.repeat_track(false)?;
        }
        spirc.repeat(on)
    })
}

/// Plays a context (`spotify:playlist:…`, `spotify:album:…`) here. Spotify
/// ignores a load sent to a device that is not active, so this activates it
/// first.
#[tauri::command]
pub fn music_play_context(app: AppHandle, uri: String) -> Result<(), String> {
    if !uri.starts_with("spotify:") {
        return Err(format!("Not a Spotify URI: {uri}"));
    }
    state::with_spirc(&app, |spirc| {
        spirc.activate()?;
        spirc.load(LoadRequest::from_context_uri(
            uri,
            LoadRequestOptions {
                start_playing: true,
                ..Default::default()
            },
        ))
    })?;
    state::update(&app, |s| s.error = None);
    Ok(())
}

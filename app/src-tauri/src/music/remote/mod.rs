//! Remote control of the Connect device playing elsewhere (a phone, the
//! desktop app): while another device holds playback, the player's commands
//! go to it rather than pulling playback here (`control.rs`), and the
//! snapshot's `remote` shows what it plays (`playback.rs`).
//!
//! Skipping to a track up next and removing one go the same way while
//! another device plays (`queue.rs`).
//!
//! A command is one request through the library's spclient sender, from this
//! device's id to the active one's: Spotify's connect-state player command
//! API, or its volume endpoint (`command.rs`). It is sent once; a non-2xx
//! fails the command with the device's name and is logged, and nothing falls
//! back to pulling playback here.

mod command;
mod playback;
#[cfg(test)]
mod tests;

use tauri::AppHandle;

pub(super) use command::{ListedTrack, RemoteCommand};
pub(super) use playback::{describe, from_cluster, with_row, RemotePlayback};

use super::library::spclient::{self, Method, Target};
use super::library::LibraryError;
use super::state;

/// The device a remote command goes to, as the last cluster push left it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Device {
    /// This device's Connect id.
    pub(super) from: String,
    /// The active device's.
    pub(super) to: String,
    pub(super) name: String,
    /// Its player state says it is playing.
    pub(super) playing: bool,
    /// Its player state allows going back a track.
    pub(super) can_prev: bool,
}

/// Sends `command` to `device`.
pub(super) async fn send(
    app: &AppHandle,
    device: &Device,
    command: RemoteCommand,
) -> Result<(), String> {
    let session = state::session(app).ok_or("Not connected to Spotify")?;
    let endpoint = command::path(&command, &device.from, &device.to);
    let body = serde_json::to_vec(&command::body(&command)).map_err(|e| e.to_string())?;
    let method = match &command {
        RemoteCommand::Volume(_) => Method::PutJson(body),
        _ => Method::PostJson(body),
    };
    match spclient::send(&session, Target::SpClient(&endpoint), method).await {
        Ok(_) => Ok(()),
        Err(e) => {
            elog!("[music] {} on {} failed: {e}", command.label(), device.name);
            Err(refused(&device.name, &e))
        }
    }
}

/// A failed remote command, worded for the player.
pub(super) fn refused(device: &str, e: &LibraryError) -> String {
    match e {
        LibraryError::Status(code) => format!("Spotify refused the command on {device} ({code})"),
        LibraryError::Unauthorized => format!("Spotify refused the command on {device} (401)"),
        LibraryError::Forbidden => format!("Spotify refused the command on {device} (403)"),
        other => other.to_string(),
    }
}

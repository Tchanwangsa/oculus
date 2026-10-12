//! Spotify's reusable credentials, kept in oculus-keyd under `music.spotify`
//! as a small JSON string: the canonical username, librespot's stored
//! credentials blob (base64) and this install's Connect device id, so the
//! device keeps one identity across launches. Read back with keyd's
//! app-only `get` (`docs/auth.md`). No keychain fallback: with keyd absent
//! there is nowhere to keep them, and the failure is the snapshot's error.
//!
//! Every call is blocking and can wait on keyd's keychain prompt: run it off
//! the async runtime (`runtime::blocking::run`).

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use keyd_core::names::MUSIC_SPOTIFY;
use librespot_core::authentication::Credentials;
use librespot_protocol::authentication::AuthenticationType;
use serde::{Deserialize, Serialize};

use crate::providers::credentials::{Credentialed, KeydError};

/// What a session logs in with, and the device id it logs in as.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Saved {
    pub credentials: Credentials,
    pub device_id: String,
}

#[derive(Serialize, Deserialize)]
struct Wire {
    username: String,
    auth_data: String,
    device_id: String,
}

impl Saved {
    /// The reusable form librespot hands back after a login: the username
    /// and the stored-credentials blob `Session::auth_data` holds.
    pub(super) fn stored(username: String, auth_data: Vec<u8>, device_id: String) -> Self {
        Saved {
            credentials: Credentials {
                username: Some(username),
                auth_type: AuthenticationType::AUTHENTICATION_STORED_SPOTIFY_CREDENTIALS,
                auth_data,
            },
            device_id,
        }
    }

    pub(super) fn encode(&self) -> String {
        serde_json::to_string(&Wire {
            username: self.credentials.username.clone().unwrap_or_default(),
            auth_data: BASE64.encode(&self.credentials.auth_data),
            device_id: self.device_id.clone(),
        })
        .expect("strings serialise")
    }

    pub(super) fn decode(text: &str) -> Result<Self, String> {
        let wire: Wire = serde_json::from_str(text)
            .map_err(|e| format!("The stored Spotify login is unreadable ({e})"))?;
        let auth_data = BASE64
            .decode(wire.auth_data)
            .map_err(|e| format!("The stored Spotify login is unreadable ({e})"))?;
        if wire.username.is_empty() || auth_data.is_empty() || wire.device_id.is_empty() {
            return Err("The stored Spotify login is incomplete".into());
        }
        Ok(Saved::stored(wire.username, auth_data, wire.device_id))
    }
}

fn broker() -> Credentialed {
    Credentialed::at(&crate::library::paths::data_dir())
}

fn describe(e: KeydError) -> String {
    match e {
        KeydError::Absent => {
            "oculus-keyd is not running, so there is nowhere to keep the Spotify login".into()
        }
        other => format!("Could not reach the stored Spotify login: {other}"),
    }
}

/// What keyd holds under `music.spotify`.
pub(super) enum Loaded {
    Nothing,
    Saved(Saved),
    /// Something that does not decode; a new login replaces it.
    Unreadable(String),
}

/// The stored login. `Err` only when keyd could not be asked.
pub(super) fn load() -> Result<Loaded, String> {
    Ok(match broker().get(MUSIC_SPOTIFY).map_err(describe)? {
        Some(text) => match Saved::decode(&text) {
            Ok(saved) => Loaded::Saved(saved),
            Err(why) => Loaded::Unreadable(why),
        },
        None => Loaded::Nothing,
    })
}

pub(super) fn save(saved: &Saved) -> Result<(), String> {
    broker()
        .store(MUSIC_SPOTIFY, &saved.encode())
        .map_err(describe)
}

pub(super) fn delete() -> Result<(), String> {
    broker().delete(MUSIC_SPOTIFY).map(|_| ()).map_err(describe)
}

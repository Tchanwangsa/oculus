//! Spotify, played natively: Oculus is a Spotify Connect device ("Oculus")
//! through librespot, with audio out of CoreAudio and no Spotify app running.
//!
//! - **One session task per app** (`session.rs`), started by `music_connect`,
//!   a finished `music_login`, or at startup when Settings → Music is on
//!   (`startup.rs`). It rebuilds a dropped connection with backoff and stops
//!   for good on an auth error.
//! - **Credentials live in oculus-keyd** under `music.spotify`, read back with
//!   keyd's app-only `get` (`credentials.rs`). No librespot `Cache`: nothing of
//!   the account, the audio or the volume is written to disk.
//! - **Rust owns the state**; every change goes out whole as `music-state`
//!   (`state.rs`), the `browser-state` discipline.
//! - **The library** (`library/`) lists the account's playlists and searches
//!   through the session's own spclient host, behind a rate-limit gate.
//! - **Commands control the device playing elsewhere** over Spotify's
//!   connect-state API (`remote/`), and pull or activate playback here when
//!   nothing is active (`control.rs`); `devices.rs` lists the account's
//!   Connect devices from the pushed cluster and moves playback between them.
//! - **The output on this Mac** is the system default or a named device,
//!   switched mid-track by the player's own sink (`audio/`).
//! - **`music_duck`** is the seam a focus mode calls (`duck.rs`); no mode
//!   logic lives here.
//!
//! See `docs/music.md`.

mod audio;
pub mod commands;
mod control;
mod credentials;
mod devices;
mod duck;
mod events;
mod library;
mod login;
mod logs;
mod remote;
mod session;
pub mod startup;
mod state;
#[cfg(test)]
mod tests;
mod track;

pub use state::{shutdown, MusicState, Snapshot};

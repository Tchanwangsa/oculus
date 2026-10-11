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
//! - **The Web API** (`web/`) lists playlists and searches with Login5's token
//!   for the live session, in memory only, behind a rate-limit gate.
//! - **Commands pull playback back** when another device holds it
//!   (`control.rs`); `devices.rs` lists the account's Connect devices and
//!   moves playback between them.
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
mod login;
mod logs;
mod session;
pub mod startup;
mod state;
#[cfg(test)]
mod tests;
mod track;
mod web;

pub use state::{shutdown, MusicState, Snapshot};

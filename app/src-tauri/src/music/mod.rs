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
//! - **`music_duck`** is the seam a focus mode calls (`duck.rs`); no mode
//!   logic lives here.
//!
//! See `docs/music.md`.

pub mod commands;
mod credentials;
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

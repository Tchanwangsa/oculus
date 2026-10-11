//! Spotify, played natively: Oculus is a Spotify Connect device ("Oculus")
//! through librespot, with audio out of CoreAudio and no Spotify app running.
//!
//! - **One session task per app** (`session.rs`), started by `music_connect`
//!   or a finished `music_login`, never at startup. It rebuilds a dropped
//!   connection with backoff and stops for good on an auth error.
//! - **Credentials live in oculus-keyd** under `music.spotify`, read back with
//!   keyd's app-only `get` (`credentials.rs`). No librespot `Cache`: nothing of
//!   the account, the audio or the volume is written to disk.
//! - **Rust owns the state**; every change goes out whole as `music-state`
//!   (`state.rs`), the `browser-state` discipline.
//!
//! See `docs/music.md`.

pub mod commands;
mod credentials;
mod events;
mod login;
mod logs;
mod session;
mod state;
#[cfg(test)]
mod tests;
mod track;

pub use state::{shutdown, MusicState, Snapshot};

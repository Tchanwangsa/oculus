//! Spotify, played natively: Oculus is a Spotify Connect device ("Oculus")
//! through librespot, with audio out of CoreAudio and no Spotify app running.
//!
//! - **One session task per app** (`session/`), started by `music_connect`,
//!   a finished `music_login`, or at startup when Settings → Music is on
//!   (`startup.rs`). It notices a dropped connection within a second,
//!   rebuilds it at once (backing off only attempts that fail), resumes what
//!   Oculus was playing (`resume.rs`), and stops for good on an auth error.
//!   `music_wake`, sent whenever the player is opened, makes it check now.
//! - **What Oculus last played** is remembered in memory and in the
//!   `music_last_played` settings row (`last_played/`), shown paused after a
//!   relaunch, and what Play plays when nothing plays anywhere.
//! - **Credentials live in oculus-keyd** under `music.spotify`, read back with
//!   keyd's app-only `get` (`credentials.rs`). No librespot `Cache`: neither the
//!   login, the audio nor the volume is written to disk; the last-played row
//!   holds only the account's username, the track and its place.
//! - **Rust owns the state**; every change goes out whole as `music-state`
//!   (`state/`), the `browser-state` discipline.
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
mod last_played;
mod library;
mod login;
mod logs;
mod queue;
mod remote;
mod resume;
mod session;
pub mod startup;
mod state;
#[cfg(test)]
mod tests;
mod track;

pub use state::{shutdown, MusicState, Snapshot};

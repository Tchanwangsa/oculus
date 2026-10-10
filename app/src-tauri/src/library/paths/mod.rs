//! App data locations, resolved without a Tauri `AppHandle`.
//!
//! One definition of the app's data directory, computed from the bundle
//! identifier as Tauri does, so the CLI and the app agree on where the vault,
//! the database and `courses/` live.

mod categories;
mod course;
mod db;
mod file_types;
mod own_files;
#[cfg(test)]
mod tests;

pub use categories::*;
pub use course::*;
pub use db::*;
pub use file_types::*;
pub use own_files::*;

use std::path::PathBuf;

/// `identifier` in tauri.conf.json; a test holds them together.
pub use keyd_core::paths::IDENTIFIER;

/// Defined in `keyd_core`, because keyd's sign-in lands on it too.
pub use keyd_core::paths::CANVAS_BASE;

/// Tauri's `app.path().app_data_dir()`, reachable without an `AppHandle`; the
/// one way every module and the CLI find the data directory. Defined in
/// `keyd_core`, so `oculus-keyd` agrees.
pub fn data_dir() -> PathBuf {
    keyd_core::paths::data_dir()
}

/// `YYYY-MM-DDTHH:MM:SS` from a Unix timestamp, without a date crate.
pub use keyd_core::clock::iso8601_utc;

/// `oculus-keyd`'s endpoint (`auth/keyd/`).
pub fn keyd_socket_path(data_dir: &std::path::Path) -> PathBuf {
    keyd_core::paths::socket(data_dir)
}

//! App data locations, resolved without a Tauri `AppHandle`.
//!
//! One definition of the app's data directory, computed from the bundle
//! identifier as Tauri does, so the CLI and the app agree on where the cookie,
//! the database and `courses/` live.

mod categories;
mod course;
mod db;
mod file_types;
mod logs;
mod own_files;
mod session;
#[cfg(test)]
mod tests;

pub use categories::*;
pub use course::*;
pub use db::*;
pub use file_types::*;
pub use logs::*;
pub use own_files::*;
pub use session::*;

use std::path::PathBuf;

/// `identifier` in tauri.conf.json; a test holds them together.
pub const IDENTIFIER: &str = "com.tchan.oculus";

pub const CANVAS_BASE: &str = "https://canvas.lms.unimelb.edu.au";

/// Tauri's `app.path().app_data_dir()`, reachable without an `AppHandle`; the
/// one way every module and the CLI find the data directory.
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(IDENTIFIER)
}

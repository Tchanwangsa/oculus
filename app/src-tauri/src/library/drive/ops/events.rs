//! `drive-changed`: what a folder command did, for every open list to reload
//! (the frontend turns it into `DRIVE_CHANGED_EVENT` per subject) and for
//! open tabs to follow a moved file.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const EVENT: &str = "drive-changed";

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveChange {
    /// Every subject a path below was in, before or after.
    pub subject_ids: Vec<i64>,
    /// `{ from, to }` for each entry moved or renamed; a folder's files
    /// moved with it under the same prefix.
    pub moved: Vec<Moved>,
    /// Entries deleted (a folder's files with it).
    pub removed: Vec<String>,
    /// Folders made and copies landed.
    pub added: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Moved {
    pub from: String,
    pub to: String,
}

pub fn emit(app: &AppHandle, change: &DriveChange) {
    if let Err(e) = app.emit(EVENT, change) {
        elog!("[oculus] drive: could not announce a change: {e}");
    }
}

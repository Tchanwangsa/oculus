//! Browser downloads: every download is staged, then moved where the student
//! says once both the choice and the last byte are in.
//!
//! WebKit's download hook is synchronous and starts writing at once, so a
//! prompt cannot block it. Each download goes to
//! `downloads/.incoming/<pid>-<id>/<name>` under the data directory, never to
//! `~/Downloads`; the frontend asks where it should go meanwhile. Either order
//! of choice and finish delivers exactly once (`state.rs`).
//!
//! - **Finished is matched by URL**, oldest first; WebView2 also names the
//!   path (`correlate.rs`). macOS reports no progress, so a poller reads the
//!   staged file's size while anything is receiving (`stage.rs`).
//! - **In Oculus** goes through `import_files`, the uploads path; the
//!   frontend writes the `files` row from the outcome. **On this Mac** is a
//!   rename, or a copy across volumes (`deliver.rs`).
//! - **The list is per session**, pushed whole on `browser-downloads` to the
//!   app's webview. URLs stay in Rust (they can carry signed tokens).

pub(crate) mod commands;
mod correlate;
#[cfg(test)]
mod correlate_test;
mod deliver;
#[cfg(test)]
mod deliver_test;
mod name;
#[cfg(test)]
mod name_test;
mod stage;
mod state;
#[cfg(test)]
mod tests;

pub(super) use stage::{on_download, sweep_incoming};
pub(super) use state::Downloads;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, EventTarget};

use super::{with_state, MAIN};
use crate::library::files::uploads::ImportOutcome;

/// Where one download is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Bytes are arriving; `received` grows.
    Receiving,
    /// Every byte is in, waiting for a choice.
    Ready,
    /// Being moved to its choice.
    Delivering,
    /// Landed where `saved_to` says.
    Saved,
    /// Thrown away by the student.
    Discarded,
    /// The download failed; nothing was kept.
    Failed,
}

/// The student's answer to the Save dialog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Dest {
    /// A subject's uploads, under its code.
    Oculus {
        subject_code: String,
    },
    /// A path on the Mac, normally from `browser_download_save_panel`.
    Path {
        path: String,
    },
    Discard,
}

/// Where a saved download landed.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SavedTo {
    /// `outcome.file` is the upload to write a `files` row for.
    Oculus {
        subject_code: String,
        outcome: ImportOutcome,
    },
    Path {
        path: String,
    },
}

/// One row of the list, as the frontend sees it.
#[derive(Clone, Debug, Serialize)]
pub struct Download {
    /// Unique for the session; never reused.
    pub id: u32,
    /// The tab it belongs to: the one it started in, or the tab that opened
    /// the blank popup it came from. The tab may be closed since.
    pub tab: u32,
    /// The file name, already made safe; what the dialog and panel suggest.
    pub name: String,
    /// The host it came from, for display; absent for `blob:`/`data:`.
    pub host: Option<String>,
    pub state: State,
    /// Bytes on disk so far.
    pub received: u64,
    /// A choice made while it was still receiving.
    pub choice: Option<Dest>,
    pub saved_to: Option<SavedTo>,
    /// Why it failed, or why a delivery did not land (it is `ready` again).
    pub error: Option<String>,
    /// Whether `browser_download_retry` can fetch it again.
    pub can_retry: bool,
}

/// Everything the frontend mirrors, sent whole on every change.
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub downloads: Vec<Download>,
}

/// `downloads/.incoming/` in the data directory, on the library's volume.
pub(super) fn incoming_root() -> PathBuf {
    crate::library::paths::data_dir()
        .join("downloads")
        .join(".incoming")
}

fn snapshot(app: &AppHandle) -> Snapshot {
    with_state(app, |s| s.downloads.snapshot())
}

/// Targeted at the app's webview only, as `browser-state` is.
fn broadcast(app: &AppHandle) {
    app.emit_to(
        EventTarget::webview(MAIN),
        "browser-downloads",
        snapshot(app),
    )
    .ok();
}

/// A URL for the log: its origin only, since the rest can be a credential.
pub(super) fn origin(url: &url::Url) -> String {
    match url.origin() {
        url::Origin::Tuple(..) => url.origin().ascii_serialization(),
        url::Origin::Opaque(_) => format!("{}:", url.scheme()),
    }
}

fn host(url: &url::Url) -> Option<String> {
    match url.origin() {
        url::Origin::Tuple(_, host, _) => Some(host.to_string()),
        url::Origin::Opaque(_) => None,
    }
}

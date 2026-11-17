//! Browser downloads: every download is staged, then moved where the student
//! says once both the choice and the last byte are in (`state.rs`).
//!
//! WebKit's download hook is synchronous and starts writing at once, so a
//! prompt cannot block it. Each download goes to
//! `downloads/.incoming/<pid>-<id>/<name>` in the data directory, never to
//! `~/Downloads`, and the frontend asks where it should go meanwhile.
//!
//! - `stage.rs` is the hook; `correlate.rs` matches a `Finished` to its
//!   download by staged path; `deliver.rs` moves it (In Oculus is the
//!   uploads path, `import_into` a chosen folder); `sweep.rs` clears a crash's leftovers.
//! - The list is per session, pushed whole on `browser-downloads` to the
//!   app's webview. A download's URL can carry a signed token, so it is
//!   never kept, sent or logged: only its host, and a hash for matching.

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
mod sweep;
#[cfg(test)]
mod sweep_test;
#[cfg(test)]
mod tests;

pub(super) use stage::on_download;
pub(super) use state::Downloads;
pub(super) use sweep::sweep_incoming;

use std::hash::{DefaultHasher, Hash, Hasher};
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
    /// The download failed; nothing was kept. `error` says so.
    Failed,
    /// Never staged: a streaming playlist, whose lines can be signed segment
    /// URLs and which is never the file meant. `error` says so.
    Refused,
}

/// The student's answer to the Save dialog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Dest {
    /// A subject's files: `target_dir` (a data-dir-relative folder of the
    /// student's own in that subject), else its root.
    Oculus {
        subject_code: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_dir: Option<String>,
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
    /// the download-only tab it came from. The tab may be closed since; a
    /// download-only tab with no opener is itself this tab, and is closed.
    pub tab: u32,
    /// The file name, already made safe; what the dialog and panel suggest.
    pub name: String,
    /// The host it came from, for display: a `blob:`'s is its page's, and a
    /// `data:` URL has none.
    pub host: Option<String>,
    pub state: State,
    /// Bytes on disk so far; the file's size once every byte is in.
    pub received: u64,
    /// A choice made while it was still receiving.
    pub choice: Option<Dest>,
    pub saved_to: Option<SavedTo>,
    /// Why it failed or was refused, or why a delivery did not land (it is
    /// `ready` again).
    pub error: Option<String>,
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

/// The URL as a number, to match a `Finished` that names no path; the URL
/// itself is not kept.
fn url_key(url: &url::Url) -> u64 {
    let mut hasher = DefaultHasher::new();
    url.as_str().hash(&mut hasher);
    hasher.finish()
}

fn host(url: &url::Url) -> Option<String> {
    match url.origin() {
        url::Origin::Tuple(_, host, _) => Some(host.to_string()),
        url::Origin::Opaque(_) => None,
    }
}

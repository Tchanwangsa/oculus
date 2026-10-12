//! What the workspace pane is told: the `ed-workspace` event stream and the
//! command results. Names and shapes are the frontend's contract
//! (`docs/sync.md`); no ticket, `auth` or token is ever in one.

use std::sync::Arc;

use serde::Serialize;

pub const EVENT: &str = "ed-workspace";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Connecting,
    Online,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    pub state: State,
    /// Why it is offline, in words.
    pub reason: Option<String>,
}

impl Status {
    pub fn connecting() -> Self {
        Status {
            state: State::Connecting,
            reason: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEvent {
    pub challenge_id: i64,
    #[serde(flatten)]
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Body {
    Status {
        status: Status,
    },
    /// A commit landed on Ed; `ours` is false when another client (Ed's web
    /// app) made it.
    Commit {
        hash: String,
        ours: bool,
    },
    /// The workspace's files on disk changed.
    Files {
        files: Vec<String>,
    },
    Error {
        message: String,
    },
    /// Both sides changed `path` differently: Ed's version is at
    /// `server_path` beside it until `ed_workspace_resolve`.
    Conflict {
        path: String,
        #[serde(rename = "serverPath")]
        server_path: String,
    },
    /// Terminal output: base64 of the PTY's bytes, as Ed sent it.
    Term {
        id: String,
        data: String,
    },
    TermState {
        id: String,
        state: TermState,
    },
    /// Run output: base64 of stdout and stderr, merged by the PTY.
    Run {
        run_id: String,
        data: String,
    },
    /// The run is over: exactly one per run.
    RunExit {
        run_id: String,
        code: Option<i64>,
        signal: Option<String>,
        /// Why it ended without an exit code, in words.
        message: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TermState {
    Running,
    Exited,
}

/// `ed_terminal_open`'s answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TerminalOpened {
    /// `term-<u32>`.
    pub id: String,
    /// A persisting terminal the server already ran was re-attached.
    pub resumed: bool,
}

/// `ed_workspace_run`'s answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStarted {
    pub run_id: String,
}

/// Where a session's events go: the main window in the app, a list in tests.
pub type Sink = Arc<dyn Fn(WorkspaceEvent) + Send + Sync>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    pub run: bool,
    pub mark: bool,
    pub terminal: bool,
    pub feedback: bool,
    pub confirm_submit: bool,
}

/// `ed_workspace_open`'s answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub challenge_id: i64,
    /// Data-dir-relative: `courses/<CODE>/lessons/<id>-<slug>/<NN>-<slide-slug>`.
    pub slide_dir: String,
    /// Workspace-relative, sorted, as on disk.
    pub files: Vec<String>,
    pub run_command: Option<String>,
    pub features: Features,
    pub due_at: Option<String>,
    pub status: Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SavedTo {
    /// Uploaded: Ed holds it as a commit.
    Ed,
    /// Offline: kept in the library only.
    Local,
}

/// `ed_workspace_save`'s answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Saved {
    pub saved: SavedTo,
    /// Ed's commit, when uploaded.
    pub hash: Option<String>,
}

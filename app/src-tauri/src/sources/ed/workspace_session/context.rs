//! What a session's socket thread and its commands share: where the
//! workspace is, its status, the commit tracker, and the helpers both sides
//! write through.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use sha2::{Digest, Sha256};

use super::commits::Commits;
use super::events::{Body, Sink, State, Status, WorkspaceEvent};
use super::paths;
use super::queue::Queue;
use super::reconcile::Puller;
use super::snapshot::Snapshots;
use super::terminal::Terminals;
use crate::sources::ed::workspace_state::{self, WorkspaceState};
use crate::sources::ed::{sahara, Ed};

/// A change made while the challenge had no sync record. The first connect
/// keeps it over the server's copy and sends it to Ed (`bootstrap`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unrecorded {
    Saved,
    Deleted,
}

pub struct Shared {
    pub status: Status,
    /// Ed's workspace and its server's host, while online.
    pub online: Option<(String, String)>,
    pub commits: Commits,
    pub unrecorded: BTreeMap<String, Unrecorded>,
    pub terminals: Terminals,
    /// Commits asked of the server for Run and Mark, awaiting their hash.
    pub snapshots: Snapshots,
}

pub struct Context {
    pub challenge_id: i64,
    pub data_dir: PathBuf,
    /// Data-dir-relative.
    pub slide_dir: String,
    pub ed: Arc<Ed>,
    pub queue: Arc<Queue>,
    /// The workspace server's origin in place of `https://<host>`: tests
    /// point it at a local fake.
    pub sahara_origin: Option<String>,
    pub puller: Puller,
    shared: Mutex<Shared>,
    sink: Sink,
}

impl Context {
    pub fn new(
        challenge_id: i64,
        data_dir: PathBuf,
        slide_dir: String,
        ed: Arc<Ed>,
        sink: Sink,
    ) -> Self {
        Context {
            challenge_id,
            data_dir,
            slide_dir,
            ed,
            queue: Arc::default(),
            sahara_origin: None,
            puller: Puller::default(),
            shared: Mutex::new(Shared {
                status: Status::connecting(),
                online: None,
                commits: Commits::default(),
                unrecorded: BTreeMap::new(),
                terminals: Terminals::default(),
                snapshots: Snapshots::default(),
            }),
            sink,
        }
    }

    /// Where the workspace server on `host` is reached.
    pub fn origin(&self, host: &str) -> String {
        self.sahara_origin
            .clone()
            .unwrap_or_else(|| sahara::origin(host))
    }

    pub fn shared(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn emit(&self, body: Body) {
        (self.sink)(WorkspaceEvent {
            challenge_id: self.challenge_id,
            body,
        });
    }

    /// The workspace folder.
    pub fn root(&self) -> PathBuf {
        self.data_dir.join(&self.slide_dir)
    }

    pub fn files(&self) -> Vec<String> {
        paths::list(&self.root())
    }

    pub fn emit_files(&self) {
        self.emit(Body::Files {
            files: self.files(),
        });
    }

    /// Set the status, telling the pane when it changed. Anything but online
    /// forgets the workspace, so a save stays local until the next connect.
    pub fn set_status(&self, state: State, reason: Option<String>) {
        let status = Status { state, reason };
        {
            let mut shared = self.shared();
            if state != State::Online {
                shared.online = None;
                shared.snapshots.fail_all();
            }
            if shared.status == status {
                return;
            }
            shared.status = status.clone();
        }
        self.emit(Body::Status { status });
    }

    pub fn status(&self) -> Status {
        self.shared().status.clone()
    }

    /// Ed's workspace id and host, when the session is online.
    pub fn online(&self) -> Option<(String, String)> {
        let shared = self.shared();
        match shared.status.state {
            State::Online => shared.online.clone(),
            _ => None,
        }
    }

    /// Upload one file and return Ed's commit, noted as ours (`commits`).
    pub fn upload(
        &self,
        workspace_id: &str,
        host: &str,
        path: &str,
        bytes: &[u8],
    ) -> Result<String, String> {
        self.shared().commits.begin();
        let uploaded = self
            .ed
            .upload_ticket(workspace_id)
            .and_then(|ticket| sahara::upload(&self.origin(host), &ticket, path, bytes));
        self.shared().commits.finish(uploaded.as_deref().ok());
        uploaded
    }

    /// A `new_commit` frame: tell the pane, and pull another client's
    /// change into the library (`reconcile::pull`).
    pub fn commit_arrived(&self, hash: String) {
        let ours = self.shared().commits.arrived(&hash);
        if !ours {
            self.puller.want(&hash);
        }
        self.emit(Body::Commit { hash, ours });
    }

    /// Change the sync record and save it. A record that is missing or
    /// unreadable starts again from this session's workspace.
    pub fn record(
        &self,
        workspace_id: &str,
        change: impl FnOnce(&mut WorkspaceState),
    ) -> Result<(), String> {
        let mut state =
            workspace_state::load(&self.data_dir, self.challenge_id).unwrap_or_else(|| {
                WorkspaceState {
                    workspace_id: workspace_id.to_string(),
                    slide_dir: self.slide_dir.clone(),
                    ..WorkspaceState::default()
                }
            });
        change(&mut state);
        workspace_state::save(&self.data_dir, self.challenge_id, &state)
    }

    pub fn has_record(&self) -> bool {
        workspace_state::has_state(&self.data_dir, self.challenge_id)
    }

    /// Whether the record holds `path` as a folder: files under it and not
    /// the path itself. A remove frame must say which it is.
    pub fn recorded_folder(&self, path: &str) -> bool {
        let folder = format!("{path}/");
        workspace_state::load(&self.data_dir, self.challenge_id).is_some_and(|state| {
            !state.files.contains_key(path) && state.files.keys().any(|p| p.starts_with(&folder))
        })
    }
}

/// The SHA-256 of `bytes`, lower-case hex, as the sync record keeps it.
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Replace `target` whole (`runtime::atomic_write`), making its folders. The
/// temporary sibling is hidden, so the file list never shows it.
pub fn write_file(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = target.parent().ok_or("the workspace file has no folder")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("could not make the folder: {e}"))?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = parent.join(format!(
        ".{name}.oculus-{}-{}",
        std::process::id(),
        crate::runtime::clock::now_nanos()
    ));
    crate::runtime::atomic_write::write(target, &tmp, bytes)
}

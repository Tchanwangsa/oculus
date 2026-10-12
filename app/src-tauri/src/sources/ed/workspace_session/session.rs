//! One open challenge: its socket thread and the file operations the pane
//! asks for. The library copy is the working copy, so every save and delete
//! happens on disk first; while online it then goes to Ed and into the sync
//! record. Saves and deletes run one at a time in the order asked (`queue`).

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use super::context::{forget, sha256, write_file, Context, Unrecorded};
use super::events::{Saved, SavedTo, Sink, WorkspaceInfo};
use super::frames;
use super::paths;
use super::queue::Place;
use super::run::Runs;
use super::slide::Slide;
use super::socket::{self, Out};
use crate::sources::ed::Ed;

pub struct Session {
    pub(super) core: Arc<Context>,
    pub(super) slide: Slide,
    pub(super) out: Sender<Out>,
    pub(super) runs: Arc<Runs>,
    /// Held while a Mark is in flight, so a second click cannot submit twice.
    pub(super) marking: Mutex<()>,
}

impl Session {
    /// Start connecting in the background; the status says how it goes.
    pub fn open(
        challenge_id: i64,
        data_dir: PathBuf,
        slide: Slide,
        ed: Arc<Ed>,
        sink: Sink,
    ) -> Result<Self, String> {
        let core = Arc::new(Context::new(
            challenge_id,
            data_dir,
            slide.slide_dir.clone(),
            ed,
            sink,
        ));
        let (out, queue) = mpsc::channel();
        let thread_core = core.clone();
        std::thread::Builder::new()
            .name(format!("ed-workspace-{challenge_id}"))
            .spawn(move || socket::run(thread_core, queue))
            .map_err(|e| format!("could not start the workspace session: {e}"))?;
        Ok(Session::with(core, slide, out))
    }

    fn with(core: Arc<Context>, slide: Slide, out: Sender<Out>) -> Self {
        Session {
            core,
            slide,
            out,
            runs: Arc::default(),
            marking: Mutex::new(()),
        }
    }

    /// A session on `core` with no socket thread: the frames it would send
    /// arrive on the receiver.
    #[cfg(test)]
    pub fn detached(core: Arc<Context>, slide: Slide) -> (Self, mpsc::Receiver<Out>) {
        let (out, queue) = mpsc::channel();
        (Session::with(core, slide, out), queue)
    }

    pub fn info(&self) -> WorkspaceInfo {
        WorkspaceInfo {
            challenge_id: self.core.challenge_id,
            slide_dir: self.slide.slide_dir.clone(),
            files: self.core.files(),
            run_command: self.slide.run_command.clone(),
            features: self.slide.features,
            due_at: self.slide.due_at.clone(),
            status: self.core.status(),
        }
    }

    /// End the socket thread, which closes the socket on its next turn, and
    /// stop a run in progress.
    pub fn close(&self) {
        let _ = self.out.send(Out::Stop);
        self.runs.stop_current();
    }

    /// A place for a save or delete, taken when the pane asks.
    pub fn place(&self) -> Place {
        self.core.queue.place()
    }

    /// The library copy of `path`, as text.
    pub fn read(&self, path: &str) -> Result<String, String> {
        let target = paths::resolve(&self.core.root(), path)?;
        let bytes = std::fs::read(&target).map_err(|e| format!("could not read {path}: {e}"))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Write `path` in the library, then, while online, upload it and record
    /// Ed's commit as the new base. A failed upload is an error, but the
    /// file stays written.
    pub fn save(&self, place: Place, path: &str, bytes: &[u8]) -> Result<Saved, String> {
        let _turn = place.wait();
        let core = &self.core;
        let target = paths::resolve(&core.root(), path)?;
        if target.is_dir() {
            return Err(format!("{path} is a folder"));
        }
        let new = !target.exists();
        write_file(&target, bytes).map_err(|e| format!("could not save {path}: {e}"))?;
        if new {
            core.emit_files();
        }
        let Some((workspace_id, host)) = core.online() else {
            if !core.has_record() {
                core.shared()
                    .unrecorded
                    .insert(path.to_string(), Unrecorded::Saved);
            }
            return Ok(Saved {
                saved: SavedTo::Local,
                hash: None,
            });
        };
        let hash = core
            .upload(&workspace_id, &host, path, bytes)
            .map_err(|e| format!("{path} is saved here, but Ed did not take it: {e}"))?;
        core.record(&workspace_id, |state| {
            state.files.insert(path.to_string(), sha256(bytes));
            state.base_commit = Some(hash.clone());
        })
        .map_err(|e| format!("{path} is saved on Ed, but its sync record is not: {e}"))?;
        Ok(Saved {
            saved: SavedTo::Ed,
            hash: Some(hash),
        })
    }

    /// Remove `path` (a file or a folder) from the library, and from Ed
    /// while online.
    pub fn delete(&self, place: Place, path: &str) -> Result<(), String> {
        let _turn = place.wait();
        let core = &self.core;
        let target = paths::resolve(&core.root(), path)?;
        let folder = target.is_dir() || (!target.exists() && core.recorded_folder(path));
        let removed = if folder {
            std::fs::remove_dir_all(&target)
        } else {
            std::fs::remove_file(&target)
        };
        match removed {
            Ok(()) => core.emit_files(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("could not delete {path}: {e}")),
        }
        let Some((workspace_id, _)) = core.online() else {
            if !core.has_record() {
                core.shared()
                    .unrecorded
                    .insert(path.to_string(), Unrecorded::Deleted);
            }
            return Ok(());
        };
        self.out
            .send(Out::Frame(frames::remove(path, folder)))
            .map_err(|_| "the workspace session has ended".to_string())?;
        core.record(&workspace_id, |state| forget(state, path))
            .map_err(|e| format!("{path} is deleted, but its sync record is not updated: {e}"))
    }
}

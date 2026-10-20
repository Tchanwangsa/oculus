//! One shared canvas: the canonical document, its revision and epoch, client
//! receipts, the hash of the bytes last read from or written to disk, and the
//! lifecycle. Every mutation runs under the session's one lock, so a commit is
//! validate + apply + persist + bump with nothing interleaved.

use super::disk::{self, Hash};
use super::error::{AuthorityError, Code};
use super::events::{Change, Event, Phase, State as PhaseState, Subscribers};
use super::receipts::{Lookup, Receipt, Receipts};
use super::rows::FileRows;
use super::{format, model, transaction};
use crate::library::files::ImportedFile;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// Real canvases embed up to ~64 MiB of images; this bounds one document.
pub const MAX_DOCUMENT_BYTES: usize = 256 * 1024 * 1024;

/// The document as it stands, in the exact bytes the serializer writes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub epoch: String,
    pub revision: u64,
    pub text: String,
    pub relative_path: String,
    pub paused: bool,
}

/// The answer to a commit. A replay carries the original revision and no
/// transaction: the client still holds the op it sent, and receipts stay small.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ack {
    pub epoch: String,
    pub revision: u64,
    pub seq: u64,
    pub op_id: String,
    pub replayed: bool,
    /// What changed and was broadcast; `None` for a replay or a no-op.
    pub transaction: Option<Value>,
}

/// One client operation.
#[derive(Debug, Clone)]
pub struct Op {
    pub client_id: String,
    pub seq: u64,
    pub op_id: String,
    pub transaction: Value,
    /// The desktop's view, saved with the next write; never a content change.
    pub viewport: Option<Value>,
    /// When given, the op is refused unless it is this session's epoch.
    pub epoch: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Resolution {
    /// Make the file on disk the canonical document.
    AdoptDisk,
    /// Overwrite the file on disk with the canonical document.
    KeepOurs,
}

/// Where a session reports to.
#[derive(Clone, Copy)]
pub struct Hub<'a> {
    pub events: &'a Subscribers,
    pub rows: &'a dyn FileRows,
}

pub struct Session {
    state: Mutex<State>,
}

pub struct State {
    file_id: i64,
    root: PathBuf,
    rel: String,
    document: Value,
    revision: u64,
    epoch: String,
    disk: Hash,
    phase: Phase,
    receipts: Receipts,
    viewport_pending: bool,
}

impl Session {
    /// Start from `text`, which must be the bytes on disk.
    pub fn open(file_id: i64, root: &Path, rel: &str, text: &str) -> Result<Self, AuthorityError> {
        let bytes = disk::read(root, rel)
            .map_err(AuthorityError::io)?
            .ok_or_else(|| AuthorityError::io("Canvas no longer exists"))?;
        let disk = disk::hash(&bytes);
        if disk != disk::hash(text.as_bytes()) {
            return Err(AuthorityError::external());
        }
        let document = parse(text)?;
        Ok(Self {
            state: Mutex::new(State {
                file_id,
                root: root.to_path_buf(),
                rel: rel.into(),
                document,
                revision: 0,
                epoch: disk::epoch(),
                disk,
                phase: Phase::Live,
                receipts: Receipts::default(),
                viewport_pending: false,
            }),
        })
    }

    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn parse(text: &str) -> Result<Value, AuthorityError> {
    let document: Value = serde_json::from_str(text)
        .map_err(|_| AuthorityError::invalid("Canvas file contains invalid JSON."))?;
    model::validate(&document).map_err(AuthorityError::invalid)?;
    Ok(document)
}

impl State {
    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn owns(&self, root: &Path, rel: &str) -> bool {
        self.phase != Phase::Ended && self.root == root && self.rel == rel
    }

    pub fn snapshot(&self) -> Result<Snapshot, AuthorityError> {
        if self.phase == Phase::Ended {
            return Err(AuthorityError::ended());
        }
        Ok(Snapshot {
            epoch: self.epoch.clone(),
            revision: self.revision,
            text: format::serialize(&self.document).map_err(AuthorityError::invalid)?,
            relative_path: self.rel.clone(),
            paused: self.phase == Phase::Paused,
        })
    }

    fn live(&self) -> Result<(), AuthorityError> {
        match self.phase {
            Phase::Live => Ok(()),
            Phase::Paused => Err(AuthorityError::external()),
            Phase::Ended => Err(AuthorityError::ended()),
        }
    }

    fn ack(&self, op: &Op, revision: u64, transaction: Option<Value>) -> Ack {
        Ack {
            epoch: self.epoch.clone(),
            revision,
            seq: op.seq,
            op_id: op.op_id.clone(),
            replayed: false,
            transaction,
        }
    }

    pub fn commit(&mut self, op: Op, hub: Hub) -> Result<Ack, AuthorityError> {
        self.live()?;
        if op.epoch.as_ref().is_some_and(|epoch| *epoch != self.epoch) {
            return Err(AuthorityError::ended());
        }
        if op.client_id.is_empty() || op.op_id.is_empty() {
            return Err(AuthorityError::invalid(
                "An operation needs a client and an id.",
            ));
        }
        match self.receipts.lookup(&op.client_id, op.seq, &op.op_id) {
            Lookup::Fresh => {}
            Lookup::Replay(revision) => {
                return Ok(Ack {
                    replayed: true,
                    ..self.ack(&op, revision, None)
                })
            }
            Lookup::Reused => {
                return Err(AuthorityError::invalid(
                    "This operation number was already used for another change.",
                ))
            }
            Lookup::Stale => return Err(AuthorityError::new(
                Code::StaleOp,
                "This change is too old to confirm. Reload the shared canvas; your draft is kept.",
            )),
        }
        if let Some(viewport) = &op.viewport {
            model::validate_viewport(Some(viewport)).map_err(AuthorityError::invalid)?;
        }
        let (mut next, effective) = transaction::apply(&self.document, &op.transaction)?;
        let viewport = op
            .viewport
            .as_ref()
            .filter(|v| !transaction::same_value(v, &self.document["viewport"]));
        if !transaction::has_changes(&effective) {
            if let Some(viewport) = viewport {
                self.document["viewport"] = viewport.clone();
                self.viewport_pending = true;
            }
            self.record(&op);
            return Ok(self.ack(&op, self.revision, None));
        }
        if let Some(viewport) = viewport {
            next["viewport"] = viewport.clone();
        }
        let text = format::serialize(&next).map_err(AuthorityError::invalid)?;
        if text.len() > MAX_DOCUMENT_BYTES {
            return Err(AuthorityError::new(
                Code::TooLarge,
                "This change would make the canvas too large to save.",
            ));
        }
        let size = self.persist(&text, hub)?;
        self.document = next;
        self.revision += 1;
        self.viewport_pending = false;
        self.record(&op);
        hub.rows.touched(self.file_id, size);
        let transaction = Arc::new(effective);
        hub.events.emit(&Event::Change(Change {
            file_id: self.file_id,
            epoch: self.epoch.clone(),
            revision: self.revision,
            client_id: Some(op.client_id.clone()),
            transaction: transaction.clone(),
        }));
        Ok(self.ack(&op, self.revision, Some(Arc::unwrap_or_clone(transaction))))
    }

    fn record(&mut self, op: &Op) {
        self.receipts.record(
            &op.client_id,
            Receipt {
                seq: op.seq,
                op_id: op.op_id.clone(),
                revision: self.revision,
            },
        );
    }

    /// Overwrite the file with `text` if it still holds the bytes last seen.
    /// A changed file pauses the session; a vanished one ends it. Another
    /// process can still write between the check and the rename: this is a
    /// guard against cooperative editors, not a cross-process compare-and-swap.
    fn persist(&mut self, text: &str, hub: Hub) -> Result<u64, AuthorityError> {
        match disk::read(&self.root, &self.rel).map_err(AuthorityError::io)? {
            None => {
                self.end(hub);
                return Err(AuthorityError::new(
                    Code::SessionEnded,
                    "The canvas file was removed.",
                ));
            }
            Some(bytes) if disk::hash(&bytes) != self.disk => {
                self.set_phase(Phase::Paused, hub);
                return Err(AuthorityError::external());
            }
            Some(_) => {}
        }
        let size = disk::write(&self.root, &self.rel, text).map_err(AuthorityError::io)?;
        self.disk = disk::hash(text.as_bytes());
        Ok(size)
    }

    fn set_phase(&mut self, phase: Phase, hub: Hub) {
        if self.phase == phase {
            return;
        }
        self.phase = phase;
        hub.events.emit(&Event::State(PhaseState {
            file_id: self.file_id,
            epoch: self.epoch.clone(),
            phase,
        }));
    }

    fn end(&mut self, hub: Hub) {
        self.set_phase(Phase::Ended, hub);
        self.document = Value::Null;
    }

    pub fn resolve(&mut self, choice: Resolution, hub: Hub) -> Result<Snapshot, AuthorityError> {
        match self.phase {
            Phase::Ended => return Err(AuthorityError::ended()),
            Phase::Live => return self.snapshot(),
            Phase::Paused => {}
        }
        let Some(bytes) = disk::read(&self.root, &self.rel).map_err(AuthorityError::io)? else {
            self.end(hub);
            return Err(AuthorityError::ended());
        };
        match choice {
            Resolution::AdoptDisk => {
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| AuthorityError::invalid("Canvas file is not UTF-8 text."))?;
                let adopted = parse(text)?;
                let change = transaction::diff(&self.document, &adopted);
                self.document = adopted;
                self.disk = disk::hash(&bytes);
                self.viewport_pending = false;
                self.set_phase(Phase::Live, hub);
                if transaction::has_changes(&change) {
                    self.revision += 1;
                    hub.rows.touched(self.file_id, bytes.len() as u64);
                    hub.events.emit(&Event::Change(Change {
                        file_id: self.file_id,
                        epoch: self.epoch.clone(),
                        revision: self.revision,
                        client_id: None,
                        transaction: Arc::new(change),
                    }));
                }
            }
            Resolution::KeepOurs => {
                self.disk = disk::hash(&bytes);
                let text = format::serialize(&self.document).map_err(AuthorityError::invalid)?;
                let size = self.persist(&text, hub)?;
                self.viewport_pending = false;
                self.set_phase(Phase::Live, hub);
                hub.rows.touched(self.file_id, size);
            }
        }
        self.snapshot()
    }

    pub fn rename(&mut self, title: &str, hub: Hub) -> Result<ImportedFile, AuthorityError> {
        if self.phase == Phase::Ended {
            return Err(AuthorityError::ended());
        }
        let moved =
            super::super::rename_at(&self.root, &self.rel, title).map_err(AuthorityError::io)?;
        if moved.relative_path != self.rel {
            self.rel = moved.relative_path.clone();
            hub.rows
                .renamed(self.file_id, &moved.filename, &moved.relative_path);
        }
        Ok(moved)
    }

    /// Remove the file and end the session; a later commit cannot recreate it.
    pub fn delete(&mut self, hub: Hub) -> Result<(), AuthorityError> {
        if self.phase == Phase::Ended {
            return Err(AuthorityError::ended());
        }
        disk::remove(&self.root, &self.rel).map_err(AuthorityError::io)?;
        self.end(hub);
        hub.rows.deleted(self.file_id);
        Ok(())
    }

    /// Save a pending viewport hint, then end the epoch.
    pub fn close(&mut self, hub: Hub) {
        if self.phase == Phase::Live && self.viewport_pending {
            let saved = format::serialize(&self.document)
                .map_err(AuthorityError::invalid)
                .and_then(|text| self.persist(&text, hub));
            match saved {
                Ok(size) => hub.rows.touched(self.file_id, size),
                Err(error) => elog!(
                    "[canvas-authority] viewport for file {} not saved: {error}",
                    self.file_id
                ),
            }
        }
        self.end(hub);
    }
}

//! The download list and what each event does to it. No I/O: every move or
//! delete it decides on comes back as an `Effect` to run outside the lock.
//!
//! A download is delivered once, when it has both a choice and its last
//! byte, whichever arrives second. Discarding before the end marks it and
//! deletes on the finish. A failed download keeps no bytes. No URL is held
//! here: a download is known by its staged path, and by its URL's hash.

use std::path::{Path, PathBuf};

use super::correlate::{settle, Ended, Owed, Pending, PATHLESS_FAILURES_ARE_OURS};
use super::{Dest, Download, SavedTo, Snapshot, State};

/// What a failed download's row says. Nothing fetches it again: its URL is
/// not kept, and a form post, a `blob:` or a signed link cannot be repeated.
const DID_NOT_FINISH: &str = "the download did not finish; start it again from the page";

/// What to do with a staged file, outside the lock.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Effect {
    /// Move the staged file to `to`, then report through `delivered`.
    Deliver {
        id: u32,
        staged: PathBuf,
        to: Target,
    },
    /// Delete the staged file's folder.
    Remove(PathBuf),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Target {
    Oculus {
        subject_code: String,
    },
    /// `may_replace` only when the save panel confirmed this very path.
    Path {
        path: PathBuf,
        may_replace: bool,
    },
}

/// A download a `Finished` event settled.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Settled {
    pub id: u32,
    /// The page it arrived in.
    pub page: u32,
    pub success: bool,
    pub effects: Vec<Effect>,
}

/// Where a download came from, without its URL.
pub(super) struct Source {
    /// The page it arrives in.
    pub page: u32,
    /// The tab it is credited to (`Download::tab`).
    pub tab: u32,
    pub url_key: u64,
    pub host: Option<String>,
}

struct Entry {
    row: Download,
    /// The page it arrives in: `row.tab`, or a download-only tab's, closed.
    page: u32,
    /// `url_key` of the URL it was requested with.
    url_key: u64,
    /// Empty for a download that was never staged.
    staged: PathBuf,
    /// Its `Finished` is in, or none will come: its page need not live.
    finished: bool,
    /// Cleared from the list while discarded bytes still arrive; dropped at
    /// the finish, which deletes them.
    cleared: bool,
    /// The path the save panel confirmed for it.
    confirmed: Option<PathBuf>,
}

impl Entry {
    fn new(id: u32, source: Source, name: String, staged: PathBuf) -> Self {
        Self {
            row: Download {
                id,
                tab: source.tab,
                name,
                host: source.host,
                state: State::Receiving,
                received: 0,
                choice: None,
                saved_to: None,
                error: None,
            },
            page: source.page,
            url_key: source.url_key,
            staged,
            finished: false,
            cleared: false,
            confirmed: None,
        }
    }
}

#[derive(Default)]
pub struct Downloads {
    entries: Vec<Entry>,
    /// Pathless `Finished` events waiting for their URL group (`correlate`).
    owed: Vec<Owed>,
    next_id: u32,
    /// Whether the size poller is running.
    polling: bool,
}

impl Downloads {
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            downloads: self
                .entries
                .iter()
                .filter(|e| !e.cleared)
                .map(|e| e.row.clone())
                .collect(),
        }
    }

    /// Records a download starting and returns its id and the path to stage
    /// it at, inside its own folder under `root`.
    pub(super) fn request(&mut self, source: Source, name: String, root: &Path) -> (u32, PathBuf) {
        let id = self.next();
        // The process id keeps two running copies of the app apart (`sweep`).
        let staged = root
            .join(format!("{}-{id}", std::process::id()))
            .join(&name);
        self.entries
            .push(Entry::new(id, source, name, staged.clone()));
        (id, staged)
    }

    /// Records a download that is not staged, with the reason its row gives.
    pub(super) fn refuse(&mut self, source: Source, name: String, reason: &str) -> u32 {
        let id = self.next();
        let mut entry = Entry::new(id, source, name, PathBuf::new());
        entry.row.state = State::Refused;
        entry.row.error = Some(reason.into());
        entry.finished = true;
        self.entries.push(entry);
        id
    }

    /// A requested download that could not be given a place to arrive in:
    /// failed from the start, with nothing staged and no `Finished` to match.
    pub(super) fn unstaged(&mut self, id: u32, reason: &str) {
        if let Some(e) = self.entry_mut(id) {
            e.row.state = State::Failed;
            e.row.error = Some(reason.into());
            e.staged = PathBuf::new();
            e.finished = true;
        }
    }

    fn next(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    fn forget(&mut self, id: u32) {
        self.entries.retain(|e| e.row.id != id);
    }

    /// True when the poller should be started: something is receiving and
    /// none runs.
    pub(super) fn start_polling(&mut self) -> bool {
        let start = !self.polling && self.entries.iter().any(|e| e.row.state == State::Receiving);
        self.polling |= start;
        start
    }

    /// What the poller should measure, or `None` — and the poller stops —
    /// when nothing is receiving.
    pub(super) fn poll(&mut self) -> Option<Vec<(u32, PathBuf)>> {
        let receiving: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.row.state == State::Receiving)
            .map(|e| (e.row.id, e.staged.clone()))
            .collect();
        if receiving.is_empty() {
            self.polling = false;
            return None;
        }
        Some(receiving)
    }

    /// The poller could not start.
    pub(super) fn stop_polling(&mut self) {
        self.polling = false;
    }

    /// Takes measured sizes; true when any row changed.
    pub(super) fn set_received(&mut self, sizes: &[(u32, u64)]) -> bool {
        let mut changed = false;
        for &(id, size) in sizes {
            if let Some(e) = self.entry_mut(id) {
                if e.row.state == State::Receiving && e.row.received != size {
                    e.row.received = size;
                    changed = true;
                }
            }
        }
        changed
    }

    /// Whether a download in `page` has not finished, so the page must live
    /// to hear its end.
    pub(in crate::shell::browser) fn unfinished_in(&self, page: u32) -> bool {
        self.entries.iter().any(|e| e.page == page && !e.finished)
    }

    /// A `Finished` event. `size` measures a staged file. Returns each
    /// download it settled; none for an event that is not ours, or one still
    /// waiting for its URL group (`correlate`).
    pub(super) fn finish(
        &mut self,
        ended: Ended<'_>,
        size: impl Fn(&Path) -> Option<u64>,
    ) -> Vec<Settled> {
        self.finish_as(ended, PATHLESS_FAILURES_ARE_OURS, size)
    }

    /// `finish`, with the platform's reading of a pathless failure given.
    pub(super) fn finish_as(
        &mut self,
        ended: Ended<'_>,
        pathless_failures_are_ours: bool,
        size: impl Fn(&Path) -> Option<u64>,
    ) -> Vec<Settled> {
        let pending: Vec<Pending<'_>> = self
            .entries
            .iter()
            .filter(|e| !e.finished)
            .map(|e| Pending {
                id: e.row.id,
                page: e.page,
                url_key: e.url_key,
                staged: &e.staged,
            })
            .collect();
        let settled = settle(&pending, &mut self.owed, ended, pathless_failures_are_ours);
        settled
            .into_iter()
            .filter_map(|(id, success)| self.end(id, success, &size))
            .collect()
    }

    /// One download's last byte, or its failure.
    fn end(
        &mut self,
        id: u32,
        success: bool,
        size: &impl Fn(&Path) -> Option<u64>,
    ) -> Option<Settled> {
        let e = self.entry_mut(id)?;
        e.finished = true;
        let (page, cleared) = (e.page, e.cleared);
        let discarded = e.row.state == State::Discarded;
        let effects = if !success || discarded {
            if !discarded {
                e.row.state = State::Failed;
                e.row.error = Some(DID_NOT_FINISH.into());
                e.row.choice = None;
            }
            vec![Effect::Remove(e.staged.clone())]
        } else {
            if let Some(n) = size(&e.staged) {
                e.row.received = n;
            }
            e.row.state = State::Ready;
            match e.row.choice.clone() {
                Some(dest) => vec![Self::start(e, dest)],
                None => vec![],
            }
        };
        if cleared {
            self.forget(id);
        }
        Some(Settled {
            id,
            page,
            success,
            effects,
        })
    }

    /// The student's choice. Delivered now if every byte is in, else kept
    /// for the finish. A download that was delivered, thrown away, failed or
    /// refused takes no choice.
    pub(super) fn choose(&mut self, id: u32, dest: Dest) -> Result<Vec<Effect>, String> {
        match &dest {
            Dest::Oculus { subject_code } if subject_code.trim().is_empty() => {
                return Err("pick a subject first".into());
            }
            Dest::Path { path } if !Path::new(path).is_absolute() => {
                return Err(format!("{path} is not a full path"));
            }
            _ => {}
        }
        let e = self.entry_mut(id).ok_or("no such download")?;
        match e.row.state {
            State::Receiving => {
                if dest == Dest::Discard {
                    e.row.state = State::Discarded;
                    e.row.choice = None;
                } else {
                    e.row.choice = Some(dest);
                }
                e.row.error = None;
                Ok(vec![])
            }
            State::Ready => Ok(vec![Self::start(e, dest)]),
            State::Delivering => Err("it is being saved".into()),
            State::Saved => Err("it is saved already".into()),
            State::Discarded => Err("it was discarded".into()),
            State::Failed => Err("it failed; start it again from the page".into()),
            State::Refused => Err("it was not downloaded".into()),
        }
    }

    /// A ready download leaves for `dest`.
    fn start(e: &mut Entry, dest: Dest) -> Effect {
        e.row.error = None;
        let to = match dest {
            Dest::Discard => {
                e.row.state = State::Discarded;
                e.row.choice = None;
                return Effect::Remove(e.staged.clone());
            }
            Dest::Oculus { ref subject_code } => Target::Oculus {
                subject_code: subject_code.clone(),
            },
            Dest::Path { ref path } => {
                let path = PathBuf::from(path);
                Target::Path {
                    may_replace: e.confirmed.as_ref() == Some(&path),
                    path,
                }
            }
        };
        e.row.state = State::Delivering;
        e.row.choice = Some(dest);
        Effect::Deliver {
            id: e.row.id,
            staged: e.staged.clone(),
            to,
        }
    }

    /// How a delivery went. One that did not land is ready again, with the
    /// reason, for another choice.
    pub(super) fn delivered(&mut self, id: u32, result: Result<SavedTo, String>) {
        let Some(e) = self.entry_mut(id) else {
            return;
        };
        match result {
            Ok(saved) => {
                e.row.state = State::Saved;
                e.row.saved_to = Some(saved);
            }
            Err(error) => {
                e.row.state = State::Ready;
                e.row.choice = None;
                e.row.error = Some(error);
            }
        }
    }

    /// The save panel's answer for `id`, which may replace a file there.
    pub(super) fn confirm(&mut self, id: u32, path: PathBuf) {
        if let Some(e) = self.entry_mut(id) {
            e.confirmed = Some(path);
        }
    }

    pub(super) fn name(&self, id: u32) -> Option<String> {
        self.entry(id).map(|e| e.row.name.clone())
    }

    /// Removes a settled row from the list. One discarded while its bytes
    /// still arrive only leaves the list: its end must still be heard, to
    /// delete them and let its page go.
    pub(super) fn clear(&mut self, id: u32) -> Result<(), String> {
        let e = self.entry_mut(id).ok_or("no such download")?;
        match (e.row.state, e.finished) {
            (State::Discarded, false) => e.cleared = true,
            (State::Saved | State::Discarded | State::Failed | State::Refused, _) => {
                self.forget(id);
            }
            _ => return Err("it is not finished with yet".into()),
        }
        Ok(())
    }

    /// Where a saved download is on disk; `data_dir` resolves an upload.
    pub(super) fn saved_path(&self, id: u32, data_dir: &Path) -> Result<PathBuf, String> {
        let e = self.entry(id).ok_or("no such download")?;
        match &e.row.saved_to {
            Some(SavedTo::Path { path }) => Ok(PathBuf::from(path)),
            Some(SavedTo::Oculus { outcome, .. }) => outcome
                .file
                .as_ref()
                .map(|f| data_dir.join(&f.relative_path))
                .ok_or_else(|| "it did not land".into()),
            None => Err("it is not saved yet".into()),
        }
    }

    fn entry(&self, id: u32) -> Option<&Entry> {
        self.entries.iter().find(|e| e.row.id == id)
    }

    fn entry_mut(&mut self, id: u32) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.row.id == id)
    }
}

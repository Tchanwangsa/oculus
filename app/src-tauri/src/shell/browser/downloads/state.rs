//! The download list and what each event does to it. No I/O: every move or
//! delete it decides on comes back as an `Effect` to run outside the lock.
//!
//! A download is delivered once, when it has both a choice and its last
//! byte, whichever arrives second. Discarding before the end marks it and
//! deletes on the finish. A failed download keeps its URL for Retry, no bytes.

use std::path::{Path, PathBuf};

use super::correlate::{match_finished, Pending};
use super::{host, Dest, Download, SavedTo, Snapshot, State};

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

struct Entry {
    row: Download,
    /// The page it arrives in: `row.tab`, or a blank popup since closed.
    page: u32,
    /// As WebKit reports it, to match `Finished`. Never sent or logged.
    url: String,
    /// Kept for Retry when it is a fetchable `http(s)` URL.
    retry: Option<url::Url>,
    staged: PathBuf,
    finished: bool,
    /// The path the save panel confirmed for it.
    confirmed: Option<PathBuf>,
}

#[derive(Default)]
pub struct Downloads {
    entries: Vec<Entry>,
    next_id: u32,
    /// Whether the size poller is running.
    polling: bool,
}

impl Downloads {
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            downloads: self.entries.iter().map(|e| e.row.clone()).collect(),
        }
    }

    /// Records a download starting in `page` for `tab` and returns its id
    /// and the path to stage it at, inside its own folder under `root`.
    pub(super) fn request(
        &mut self,
        page: u32,
        tab: u32,
        url: &url::Url,
        name: String,
        root: &Path,
    ) -> (u32, PathBuf) {
        self.next_id += 1;
        let id = self.next_id;
        // The process id keeps two running copies of the app apart.
        let staged = root
            .join(format!("{}-{id}", std::process::id()))
            .join(&name);
        let retry = matches!(url.scheme(), "http" | "https").then(|| url.clone());
        self.entries.push(Entry {
            row: Download {
                id,
                tab,
                name,
                host: host(url),
                state: State::Receiving,
                received: 0,
                choice: None,
                saved_to: None,
                error: None,
                can_retry: retry.is_some(),
            },
            page,
            url: url.as_str().to_string(),
            retry,
            staged: staged.clone(),
            finished: false,
            confirmed: None,
        });
        (id, staged)
    }

    /// Drops a download outright: one that never started, or one cleared.
    pub(super) fn forget(&mut self, id: u32) {
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

    pub(super) fn page_of(&self, id: u32) -> Option<u32> {
        self.entry(id).map(|e| e.page)
    }

    /// A download ended. `size` measures the staged file. Returns which one,
    /// or `None` for one never staged here.
    pub(super) fn finish(
        &mut self,
        url: &str,
        path: Option<&Path>,
        success: bool,
        size: impl Fn(&Path) -> Option<u64>,
    ) -> Option<(u32, Vec<Effect>)> {
        let pending: Vec<Pending<'_>> = self
            .entries
            .iter()
            .filter(|e| !e.finished)
            .map(|e| Pending {
                id: e.row.id,
                url: &e.url,
                staged: &e.staged,
            })
            .collect();
        let id = match_finished(&pending, url, path)?;
        let e = self.entry_mut(id)?;
        e.finished = true;
        let discarded = e.row.state == State::Discarded;
        if !success || discarded {
            if !discarded {
                e.row.state = State::Failed;
                e.row.error = Some("the download did not finish".into());
                e.row.choice = None;
            }
            return Some((id, vec![Effect::Remove(e.staged.clone())]));
        }
        if let Some(n) = size(&e.staged) {
            e.row.received = n;
        }
        e.row.state = State::Ready;
        let effects = match e.row.choice.clone() {
            Some(dest) => vec![Self::start(e, dest)],
            None => vec![],
        };
        Some((id, effects))
    }

    /// The student's choice. Delivered now if every byte is in, else kept
    /// for the finish. A download that was delivered, thrown away or failed
    /// takes no choice.
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
            State::Failed => Err("it failed; retry it instead".into()),
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

    /// Removes a settled row from the list.
    pub(super) fn clear(&mut self, id: u32) -> Result<(), String> {
        let e = self.entry(id).ok_or("no such download")?;
        match e.row.state {
            State::Saved | State::Discarded | State::Failed => {
                self.forget(id);
                Ok(())
            }
            _ => Err("it is not finished with yet".into()),
        }
    }

    /// The tab and URL to fetch a failed download again from.
    pub(super) fn retry_target(&self, id: u32) -> Result<(u32, url::Url), String> {
        let e = self.entry(id).ok_or("no such download")?;
        if e.row.state != State::Failed {
            return Err("only a failed download can be retried".into());
        }
        let url = e
            .retry
            .clone()
            .ok_or("this download cannot be fetched again")?;
        Ok((e.row.tab, url))
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

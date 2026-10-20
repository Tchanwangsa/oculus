//! A share's change log: the accepted changes after its base revision, each
//! serialized once as the JSON a `/changes` entry is, so catch-up answers
//! join stored bytes and never re-serialize. Bounded by `LOG_REVISIONS` and
//! `LOG_BYTES`; a client behind the oldest kept revision refetches the
//! snapshot (410 `gone`).

use crate::companion::limits::{LOG_BYTES, LOG_REVISIONS};
use crate::library::files::canvases::authority::events::Change;
use bytes::Bytes;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Arc;

/// One change as `{epoch, revision, clientId, transaction}`.
pub struct Entry {
    pub epoch: String,
    pub revision: u64,
    pub json: Bytes,
}

impl Entry {
    pub fn from_change(change: &Change) -> Result<Self, serde_json::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire<'a> {
            epoch: &'a str,
            revision: u64,
            client_id: Option<&'a str>,
            transaction: &'a serde_json::Value,
        }
        let json = serde_json::to_vec(&Wire {
            epoch: &change.epoch,
            revision: change.revision,
            client_id: change.client_id.as_deref(),
            transaction: &change.transaction,
        })?;
        Ok(Self {
            epoch: change.epoch.clone(),
            revision: change.revision,
            json: Bytes::from(json),
        })
    }
}

/// `from` is older than the log, newer than the head, or of another epoch.
#[derive(Debug, PartialEq, Eq)]
pub struct Gone;

#[derive(Default)]
pub struct ChangeLog {
    /// `None` until the share's base snapshot is known; entries recorded
    /// before then are filtered against it.
    epoch: Option<String>,
    /// Changes after this revision are all kept.
    floor: u64,
    head: u64,
    entries: VecDeque<Arc<Entry>>,
    bytes: usize,
}

impl ChangeLog {
    /// Fix the base: keep only this epoch's entries after `revision`.
    pub fn establish(&mut self, epoch: &str, revision: u64) {
        self.entries
            .retain(|entry| entry.epoch == epoch && entry.revision > revision);
        self.bytes = self.entries.iter().map(|e| e.json.len()).sum();
        self.epoch = Some(epoch.to_string());
        self.floor = revision;
        self.head = self.entries.back().map_or(revision, |e| e.revision);
        self.evict();
    }

    /// Record an entry; one of another epoch or already seen is dropped.
    /// Returns whether it was kept.
    pub fn push(&mut self, entry: Arc<Entry>) -> bool {
        if let Some(epoch) = &self.epoch {
            if entry.epoch != *epoch || entry.revision <= self.head {
                return false;
            }
            self.head = entry.revision;
        }
        self.bytes += entry.json.len();
        self.entries.push_back(entry);
        self.evict();
        true
    }

    fn evict(&mut self) {
        if self.epoch.is_none() {
            return;
        }
        while self.entries.len() > LOG_REVISIONS || self.bytes > LOG_BYTES {
            let Some(oldest) = self.entries.pop_front() else {
                break;
            };
            self.bytes -= oldest.json.len();
            self.floor = oldest.revision;
        }
    }

    pub fn epoch(&self) -> Option<&str> {
        self.epoch.as_deref()
    }

    pub fn head(&self) -> u64 {
        self.head
    }

    /// The entries after `from`, for a client in `epoch` (when it names one).
    pub fn since(&self, from: u64, epoch: Option<&str>) -> Result<Vec<Arc<Entry>>, Gone> {
        let Some(own) = &self.epoch else {
            return Err(Gone);
        };
        if epoch.is_some_and(|epoch| epoch != own) || from < self.floor || from > self.head {
            return Err(Gone);
        }
        Ok(self
            .entries
            .iter()
            .filter(|entry| entry.revision > from)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn revisions(found: Result<Vec<Arc<Entry>>, Gone>) -> Option<Vec<u64>> {
        found
            .ok()
            .map(|entries| entries.iter().map(|e| e.revision).collect())
    }

    fn entry(epoch: &str, revision: u64, size: usize) -> Arc<Entry> {
        Arc::new(Entry {
            epoch: epoch.into(),
            revision,
            json: Bytes::from(vec![b' '; size]),
        })
    }

    #[test]
    fn entries_recorded_before_the_base_are_filtered_against_it() {
        let mut log = ChangeLog::default();
        assert!(log.push(entry("e", 4, 1)));
        assert!(log.push(entry("e", 5, 1)));
        assert!(log.push(entry("other", 6, 1)));
        assert_eq!(revisions(log.since(0, None)), None);
        log.establish("e", 4);
        assert_eq!(revisions(log.since(4, None)), Some(vec![5]));
        assert_eq!(log.head(), 5);
        assert!(!log.push(entry("e", 5, 1)));
        assert!(!log.push(entry("other", 6, 1)));
        assert!(log.push(entry("e", 6, 1)));
    }

    #[test]
    fn catch_up_answers_only_inside_the_log_and_the_epoch() {
        let mut log = ChangeLog::default();
        log.establish("e", 10);
        for revision in 11..=13 {
            log.push(entry("e", revision, 1));
        }
        assert_eq!(log.since(11, Some("e")).unwrap().len(), 2);
        assert_eq!(log.since(13, None).unwrap().len(), 0);
        assert_eq!(revisions(log.since(9, None)), None);
        assert_eq!(revisions(log.since(14, None)), None);
        assert_eq!(revisions(log.since(11, Some("f"))), None);
    }

    #[test]
    fn the_log_keeps_the_last_256_revisions() {
        let mut log = ChangeLog::default();
        log.establish("e", 0);
        for revision in 1..=300 {
            log.push(entry("e", revision, 1));
        }
        assert_eq!(revisions(log.since(43, None)), None);
        assert_eq!(log.since(44, None).unwrap().len(), 256);
    }

    #[test]
    fn the_log_keeps_at_most_64_mib() {
        let mut log = ChangeLog::default();
        log.establish("e", 0);
        let mib = 1024 * 1024;
        for revision in 1..=5 {
            log.push(entry("e", revision, 20 * mib));
        }
        assert_eq!(revisions(log.since(1, None)), None);
        assert_eq!(log.since(2, None).unwrap().len(), 3);
        log.push(entry("e", 6, 70 * mib));
        assert_eq!(revisions(log.since(5, None)), None);
        assert_eq!(log.since(6, None).unwrap().len(), 0);
    }
}

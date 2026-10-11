//! What the library keeps in memory, per account, and its rate-limit gate.
//! Playlists, playlist contents and made covers are dropped on a refresh;
//! headers and track rows describe things that rarely change and are kept.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::contents::Contents;
use super::error::LibraryError;
use super::gate::Gate;
use super::metadata::TrackRow;
use super::rootlist::{Header, Playlist};

/// The account a map belongs to, and the map, by URI.
pub(super) type PerAccount<V> = Option<(String, HashMap<String, V>)>;

#[derive(Default)]
pub struct Library(Mutex<Inner>);

#[derive(Default)]
pub(super) struct Inner {
    gate: Gate,
    /// The account they belong to, and its playlists.
    pub(super) playlists: Option<(String, Vec<Playlist>)>,
    /// Headers fetched for playlists the rootlist named no name for.
    pub(super) headers: PerAccount<Header>,
    /// Covers made for playlists Spotify gave none: empty for one with no
    /// track covers or one Spotify would not describe.
    pub(super) covers: PerAccount<Vec<String>>,
    /// Playlists' tracks as far as they have been read.
    pub(super) contents: PerAccount<Contents>,
    /// Track rows; `None` for a track Spotify sent nothing for.
    pub(super) tracks: PerAccount<Option<TrackRow>>,
}

/// `slot`'s value for `key` when it belongs to `account`.
pub(super) fn lookup<V: Clone>(slot: &PerAccount<V>, account: &str, key: &str) -> Option<V> {
    match slot {
        Some((owner, map)) if owner == account => map.get(key).cloned(),
        _ => None,
    }
}

/// `slot`'s map when it belongs to `account`, else a fresh one that does.
pub(super) fn map_for<'a, V>(
    slot: &'a mut PerAccount<V>,
    account: &str,
) -> &'a mut HashMap<String, V> {
    if slot.as_ref().is_none_or(|(owner, _)| owner != account) {
        *slot = Some((account.to_string(), HashMap::new()));
    }
    &mut slot.get_or_insert_with(Default::default).1
}

impl Library {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `f` under the lock.
    pub(super) fn with<T>(&self, f: impl FnOnce(&mut Inner) -> T) -> T {
        f(&mut self.lock())
    }

    pub(super) fn check_gate(&self) -> Result<(), LibraryError> {
        match self.lock().gate.closed_for(Instant::now()) {
            Some(secs) => Err(LibraryError::RateLimited(secs)),
            None => Ok(()),
        }
    }

    pub(super) fn close_gate(&self, wait: Duration) {
        self.lock().gate.close(Instant::now(), wait);
    }

    pub(super) fn playlists_for(&self, account: &str) -> Option<Vec<Playlist>> {
        match &self.lock().playlists {
            Some((owner, list)) if owner == account => Some(list.clone()),
            _ => None,
        }
    }

    /// Drops what a refresh asks for again.
    pub(super) fn forget_lists(&self) {
        let mut inner = self.lock();
        inner.covers = None;
        inner.contents = None;
    }

    /// `list` with each made cover on the playlists that have none.
    pub(super) fn with_covers(&self, account: &str, mut list: Vec<Playlist>) -> Vec<Playlist> {
        let inner = self.lock();
        for playlist in list.iter_mut().filter(|p| p.images.is_empty()) {
            if let Some(urls) = lookup(&inner.covers, account, &playlist.uri) {
                playlist.images = urls;
            }
        }
        list
    }
}

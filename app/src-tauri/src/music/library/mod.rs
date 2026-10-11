//! The account's playlists and search, from the session's own spclient host
//! with its Login5 token: the rootlist (`rootlist.rs`) and the `searchview`
//! service (`search.rs`). Spotify's public Web API is never called; its
//! shared client id is throttled. Calls stay few all the same: the playlists
//! are fetched once per account and kept until asked again, search runs only
//! when asked, a 429 closes a gate until its `Retry-After` (`gate.rs`), and
//! nothing is retried.
//!
//! See `docs/music.md`.

mod error;
mod gate;
mod rootlist;
mod search;
mod spclient;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use futures_util::{stream, StreamExt};
use librespot_core::Session;
use tauri::AppHandle;

pub(super) use error::LibraryError;
pub use rootlist::Playlist;
pub use search::SearchResults;

use super::state;
use gate::Gate;
use rootlist::{Entry, Header};
use spclient::{JSON, PROTOBUF};

/// Playlist headers fetched at once when the rootlist names none.
const HEADER_CONCURRENCY: usize = 4;

#[derive(Default)]
pub struct Library(Mutex<Inner>);

#[derive(Default)]
struct Inner {
    gate: Gate,
    /// The account they belong to, and its playlists.
    playlists: Option<(String, Vec<Playlist>)>,
    /// The account they belong to, and the headers fetched for playlists
    /// the rootlist named no name for, by URI.
    headers: Option<(String, HashMap<String, Header>)>,
}

impl Library {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn check_gate(&self) -> Result<(), LibraryError> {
        match self.lock().gate.closed_for(Instant::now()) {
            Some(secs) => Err(LibraryError::RateLimited(secs)),
            None => Ok(()),
        }
    }

    fn close_gate(&self, wait: Duration) {
        self.lock().gate.close(Instant::now(), wait);
    }

    fn playlists_for(&self, account: &str) -> Option<Vec<Playlist>> {
        match &self.lock().playlists {
            Some((owner, list)) if owner == account => Some(list.clone()),
            _ => None,
        }
    }

    fn set_playlists(&self, account: String, list: Vec<Playlist>) {
        self.lock().playlists = Some((account, list));
    }

    fn header(&self, account: &str, uri: &str) -> Option<Header> {
        match &self.lock().headers {
            Some((owner, map)) if owner == account => map.get(uri).cloned(),
            _ => None,
        }
    }

    fn keep_header(&self, account: &str, uri: String, header: Header) {
        let mut inner = self.lock();
        if inner
            .headers
            .as_ref()
            .is_none_or(|(owner, _)| owner != account)
        {
            inner.headers = Some((account.to_string(), HashMap::new()));
        }
        if let Some((_, map)) = inner.headers.as_mut() {
            map.insert(uri, header);
        }
    }
}

/// The account's playlists in rootlist order, from memory unless `refresh`
/// or the account changed. Up to `rootlist::CAP`.
pub(super) async fn playlists(
    app: &AppHandle,
    refresh: bool,
) -> Result<Vec<Playlist>, LibraryError> {
    let session = state::session(app).ok_or(LibraryError::NotConnected)?;
    let account = session.username();
    let library = state::library(app);
    if !refresh {
        if let Some(list) = library.playlists_for(&account) {
            return Ok(list);
        }
    }
    let mut entries = Vec::new();
    let mut from = 0;
    loop {
        let body = call(
            app,
            &session,
            "rootlist",
            &rootlist::endpoint(&account, from),
            PROTOBUF,
        )
        .await?;
        let page = rootlist::parse_page(&body)?;
        entries.extend(page.entries);
        from += page.items;
        if !page.truncated || page.items == 0 || entries.len() >= rootlist::CAP {
            break;
        }
    }
    entries.truncate(rootlist::CAP);
    let list = named(app, &session, &account, entries).await?;
    library.set_playlists(account, list.clone());
    Ok(list)
}

/// Each entry with its header: the rootlist's own, else one kept from an
/// earlier fetch, else its playlist's, `HEADER_CONCURRENCY` at a time. A
/// playlist Spotify will not describe is left out; a rate limit or a lost
/// connection fails the whole list.
async fn named(
    app: &AppHandle,
    session: &Session,
    account: &str,
    entries: Vec<Entry>,
) -> Result<Vec<Playlist>, LibraryError> {
    let library = state::library(app);
    let fetched: Vec<Result<Option<Playlist>, LibraryError>> = stream::iter(entries)
        .map(|entry| async move {
            if let Some(header) = entry.header {
                return Ok(Some(header.playlist(entry.uri)));
            }
            if let Some(header) = library.header(account, &entry.uri) {
                return Ok(Some(header.playlist(entry.uri)));
            }
            let Some(endpoint) = rootlist::playlist_endpoint(&entry.uri) else {
                return Ok(None);
            };
            let header = match call(app, session, "playlist", &endpoint, PROTOBUF).await {
                Ok(body) => rootlist::parse_playlist(&body),
                Err(e) => Err(e),
            };
            match header {
                Ok(Some(header)) => {
                    library.keep_header(account, entry.uri.clone(), header.clone());
                    Ok(Some(header.playlist(entry.uri)))
                }
                Ok(None)
                | Err(
                    LibraryError::Forbidden | LibraryError::Status(_) | LibraryError::Unreadable(_),
                ) => Ok(None),
                Err(e) => Err(e),
            }
        })
        .buffered(HEADER_CONCURRENCY)
        .collect()
        .await;
    fetched.into_iter().filter_map(Result::transpose).collect()
}

/// Playlists, albums and tracks for `query`; an empty query asks nothing.
pub(super) async fn search(app: &AppHandle, query: &str) -> Result<SearchResults, LibraryError> {
    let Some(query) = search::clean(query) else {
        return Ok(SearchResults::default());
    };
    let session = state::session(app).ok_or(LibraryError::NotConnected)?;
    let endpoint = search::endpoint(&query, &session.country(), &session.username());
    let body = call(app, &session, "search", &endpoint, JSON)
        .await
        .map_err(search::unavailable)?;
    search::parse(&body)
}

/// One request through the gate; a 429 closes it. `what` names it in the
/// log, which never carries the endpoint: a search's holds the query.
async fn call(
    app: &AppHandle,
    session: &Session,
    what: &str,
    endpoint: &str,
    accept: &'static str,
) -> Result<Vec<u8>, LibraryError> {
    let library = state::library(app);
    library.check_gate()?;
    let result = spclient::get(session, endpoint, accept).await;
    if let Err(e) = &result {
        if let LibraryError::RateLimited(secs) = e {
            library.close_gate(Duration::from_secs(*secs));
        }
        elog!("[music] {what}: {e}");
    }
    result
}

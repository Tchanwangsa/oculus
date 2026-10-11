//! The account's playlists, their tracks and search, from the session's own
//! spclient host with its Login5 token: the rootlist (`rootlist.rs`), a
//! playlist's contents (`contents.rs`), track metadata in batches
//! (`metadata.rs`), covers for playlists Spotify gives none (`covers.rs`)
//! and search through pathfinder (`search.rs`). Spotify's public Web API is
//! never called; its shared client id is throttled. Calls stay few all the
//! same: what is fetched is kept per account (`cache.rs`) until asked again,
//! search runs only when asked, a 429 closes a gate until its `Retry-After`
//! (`gate.rs`), and nothing is retried.
//!
//! See `docs/music-library.md`.

mod cache;
mod contents;
mod covers;
mod error;
mod gate;
mod metadata;
mod rootlist;
mod search;
mod spclient;
#[cfg(test)]
mod tests;
mod tracks;

use std::collections::HashMap;
use std::time::Duration;

use futures_util::{stream, StreamExt};
use librespot_core::Session;
use tauri::AppHandle;

pub use cache::Library;
pub(super) use error::LibraryError;
pub use metadata::TrackRow;
pub use rootlist::Playlist;
pub use search::SearchResults;

use super::state;
use cache::{lookup, map_for};
use contents::Contents;
use rootlist::Entry;
use spclient::{Method, Target, PROTOBUF};

/// Playlists read at once: headers when the rootlist names none, first
/// pages for covers.
const HEADER_CONCURRENCY: usize = 4;

/// The account's playlists in rootlist order, from memory unless `refresh`
/// or the account changed. Up to `rootlist::CAP`. Covers made earlier are
/// filled in; a refresh drops them and every playlist's contents, to be
/// read again.
pub(super) async fn playlists(
    app: &AppHandle,
    refresh: bool,
) -> Result<Vec<Playlist>, LibraryError> {
    let session = state::session(app).ok_or(LibraryError::NotConnected)?;
    let account = session.username();
    let library = state::library(app);
    if refresh {
        library.forget_lists();
    } else if let Some(list) = library.playlists_for(&account) {
        return Ok(library.with_covers(&account, list));
    }
    let mut entries = Vec::new();
    let mut from = 0;
    loop {
        let body = call(
            app,
            &session,
            "rootlist",
            &rootlist::endpoint(&account, from),
            Method::Get(PROTOBUF),
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
    library.with(|inner| inner.playlists = Some((account.clone(), list.clone())));
    Ok(library.with_covers(&account, list))
}

/// Each entry with its header: the rootlist's own, else one kept from an
/// earlier fetch, else its playlist's, `HEADER_CONCURRENCY` at a time (that
/// answer's first tracks are kept too, for its cover and its track list). A
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
            if let Some(header) = library.with(|inner| lookup(&inner.headers, account, &entry.uri))
            {
                return Ok(Some(header.playlist(entry.uri)));
            }
            let Some(endpoint) = rootlist::playlist_endpoint(&entry.uri) else {
                return Ok(None);
            };
            let body = match call(app, session, "playlist", &endpoint, Method::Get(PROTOBUF)).await
            {
                Ok(body) => body,
                Err(e) if e.is_lasting() => return Ok(None),
                Err(e) => return Err(e),
            };
            let Ok(Some(header)) = rootlist::parse_playlist(&body) else {
                return Ok(None);
            };
            let first = contents::parse_page(&body).ok().map(Contents::first);
            library.with(|inner| {
                map_for(&mut inner.headers, account).insert(entry.uri.clone(), header.clone());
                if let Some(first) = first {
                    map_for(&mut inner.contents, account).insert(entry.uri.clone(), first);
                }
            });
            Ok(Some(header.playlist(entry.uri)))
        })
        .buffered(HEADER_CONCURRENCY)
        .collect()
        .await;
    fetched.into_iter().filter_map(Result::transpose).collect()
}

/// Covers for playlists that have none, by URI (`tracks::covers`).
pub(super) async fn playlist_covers(
    app: &AppHandle,
    uris: Vec<String>,
) -> Result<HashMap<String, Vec<String>>, LibraryError> {
    tracks::covers(app, uris).await
}

/// Every catalogue track of the playlist `uri`, in order; local files and
/// episodes are left out. Kept until a refresh.
pub(super) async fn playlist_tracks(
    app: &AppHandle,
    uri: &str,
) -> Result<Vec<String>, LibraryError> {
    let session = state::session(app).ok_or(LibraryError::NotConnected)?;
    let account = session.username();
    tracks::contents(app, &session, &account, uri, true).await
}

/// Rows for these tracks, by URI, from memory or batched metadata requests.
/// A track Spotify describes nothing for is left out; a batch failing for
/// now (a rate limit, the connection) fails the call, keeping what the
/// others found for next time.
pub(super) async fn track_rows(
    app: &AppHandle,
    uris: &[String],
) -> Result<HashMap<String, TrackRow>, LibraryError> {
    let session = state::session(app).ok_or(LibraryError::NotConnected)?;
    let account = session.username();
    let found = tracks::rows(app, &session, &account, uris).await;
    match found.error {
        Some(e) => Err(e),
        None => Ok(found.rows),
    }
}

/// Playlists, albums and tracks for `query`; an empty query asks nothing.
pub(super) async fn search(app: &AppHandle, query: &str) -> Result<SearchResults, LibraryError> {
    let Some(query) = search::clean(query) else {
        return Ok(SearchResults::default());
    };
    let session = state::session(app).ok_or(LibraryError::NotConnected)?;
    let body = call_to(
        app,
        &session,
        "search",
        Target::Url(search::PATHFINDER),
        Method::PostJson(search::body(&query)),
    )
    .await
    .map_err(search::unavailable)?;
    logged("search", search::parse(&body))
}

/// One request through the gate; a 429 closes it. `what` names it in the
/// log, which never carries the endpoint: a search's holds the query.
async fn call(
    app: &AppHandle,
    session: &Session,
    what: &str,
    endpoint: &str,
    method: Method,
) -> Result<Vec<u8>, LibraryError> {
    call_to(app, session, what, Target::SpClient(endpoint), method).await
}

/// `call` to any target.
async fn call_to(
    app: &AppHandle,
    session: &Session,
    what: &str,
    target: Target<'_>,
    method: Method,
) -> Result<Vec<u8>, LibraryError> {
    let library = state::library(app);
    library.check_gate()?;
    let result = spclient::send(session, target, method).await;
    if let Err(e) = &result {
        if let LibraryError::RateLimited(secs) = e {
            library.close_gate(Duration::from_secs(*secs));
        }
        elog!("[music] {what}: {e}");
    }
    result
}

/// A body that would not decode, logged like a failed call.
fn logged<T>(what: &str, result: Result<T, LibraryError>) -> Result<T, LibraryError> {
    if let Err(e) = &result {
        elog!("[music] {what}: {e}");
    }
    result
}

//! Playlist contents, track rows and the covers made from them: each read
//! through the cache first, each request through `call` and its gate.

use std::collections::{HashMap, HashSet};

use futures_util::{stream, StreamExt};
use librespot_core::Session;
use librespot_protocol::extension_kind::ExtensionKind;
use tauri::AppHandle;

use super::cache::{lookup, map_for};
use super::contents::{self, Contents};
use super::error::LibraryError;
use super::metadata::{self, TrackRow};
use super::rootlist::{self, playlist_endpoint};
use super::spclient::{Method, PROTOBUF};
use super::{call, covers, logged, HEADER_CONCURRENCY};
use crate::music::state;

/// What a run of metadata requests found.
#[derive(Default)]
pub(super) struct Rows {
    pub(super) rows: HashMap<String, TrackRow>,
    /// Tracks whose batch failed for now: a rate limit, a token, the
    /// connection. Asking again later may answer them.
    pub(super) unanswered: HashSet<String>,
    /// The first such failure.
    pub(super) error: Option<LibraryError>,
}

/// Rows for `uris`: from memory, the rest in batches of `metadata::BATCH`.
/// A track Spotify sends nothing for, or in a batch it refuses outright, is
/// remembered as having no row.
pub(super) async fn rows(
    app: &AppHandle,
    session: &Session,
    account: &str,
    uris: &[String],
) -> Rows {
    let library = state::library(app);
    let mut found = Rows::default();
    let mut missing = Vec::new();
    let mut asked = HashSet::new();
    library.with(|inner| {
        for uri in uris {
            if !asked.insert(uri.as_str()) {
                continue;
            }
            match lookup(&inner.tracks, account, uri) {
                Some(Some(row)) => {
                    found.rows.insert(uri.clone(), row);
                }
                Some(None) => {}
                None => missing.push(uri.clone()),
            }
        }
    });
    for batch in missing.chunks(metadata::BATCH) {
        let what = "track metadata";
        let body = Method::PostProtobuf(metadata::request(batch, ExtensionKind::TRACK_V4));
        let answered = match call(app, session, what, metadata::ENDPOINT, body)
            .await
            .and_then(|body| logged(what, metadata::parse(&body)))
        {
            Ok(answered) => answered,
            Err(e) if e.is_lasting() => HashMap::new(),
            Err(e) => {
                found.unanswered.extend(batch.iter().cloned());
                found.error.get_or_insert(e);
                continue;
            }
        };
        library.with(|inner| {
            let kept = map_for(&mut inner.tracks, account);
            for uri in batch {
                kept.insert(uri.clone(), answered.get(uri).cloned());
            }
        });
        found.rows.extend(answered);
    }
    found
}

/// The playlist's tracks: its first answer, or with `whole` every page.
/// What has been read is kept until a refresh.
pub(super) async fn contents(
    app: &AppHandle,
    session: &Session,
    account: &str,
    uri: &str,
    whole: bool,
) -> Result<Vec<String>, LibraryError> {
    let library = state::library(app);
    let what = "playlist tracks";
    let kept = library.with(|inner| lookup(&inner.contents, account, uri));
    let mut list = match kept {
        Some(list) => list,
        None => {
            let endpoint = playlist_endpoint(uri).ok_or(LibraryError::Status(404))?;
            let body = call(app, session, what, &endpoint, Method::Get(PROTOBUF)).await?;
            Contents::first(logged(what, contents::parse_page(&body))?)
        }
    };
    while whole {
        let Some(from) = list.next else { break };
        let endpoint = contents::page_endpoint(uri, from).ok_or(LibraryError::Status(404))?;
        // A failed later page keeps what was read; the next open goes on.
        let page = match call(app, session, what, &endpoint, Method::Get(PROTOBUF))
            .await
            .and_then(|body| logged(what, contents::parse_page(&body)))
        {
            Ok(page) => page,
            Err(_) => break,
        };
        list.extend(from, page);
    }
    let tracks = list.tracks.clone();
    library.with(|inner| map_for(&mut inner.contents, account).insert(uri.to_string(), list));
    Ok(tracks)
}

/// Covers for playlists that have none, by URI: those made before from
/// memory, the rest from their first tracks' albums — each playlist's first
/// answer `HEADER_CONCURRENCY` at a time, then one run of metadata batches
/// for all their tracks. A playlist Spotify will not describe gets an empty
/// cover and keeps the glyph; one left unfinished by a rate limit or a lost
/// connection is left out, to be asked for again.
pub(super) async fn covers(
    app: &AppHandle,
    uris: Vec<String>,
) -> Result<HashMap<String, Vec<String>>, LibraryError> {
    let session = state::session(app).ok_or(LibraryError::NotConnected)?;
    let account = session.username();
    let library = state::library(app);
    let mut found = HashMap::new();
    let mut asked = HashSet::new();
    let wanted: Vec<String> = library.with(|inner| {
        uris.into_iter()
            .filter(|uri| asked.insert(uri.clone()))
            .filter(|uri| match lookup(&inner.covers, &account, uri) {
                Some(urls) => {
                    found.insert(uri.clone(), urls);
                    false
                }
                None => playlist_endpoint(uri).is_some(),
            })
            .take(rootlist::CAP)
            .collect()
    });
    if wanted.is_empty() {
        return Ok(found);
    }
    let (session, account) = (&session, account.as_str());
    let listed: Vec<(String, Result<Vec<String>, LibraryError>)> = stream::iter(wanted)
        .map(|uri| async move {
            let tracks = contents(app, session, account, &uri, false).await;
            (uri, tracks)
        })
        .buffered(HEADER_CONCURRENCY)
        .collect()
        .await;

    let mut made = HashMap::new();
    let mut firsts = Vec::new();
    for (uri, tracks) in listed {
        match tracks {
            Ok(mut tracks) => {
                tracks.truncate(covers::TRACKS);
                firsts.push((uri, tracks));
            }
            Err(e) if e.is_lasting() => {
                made.insert(uri, Vec::new());
            }
            Err(_) => {}
        }
    }
    let all: Vec<String> = firsts.iter().flat_map(|(_, t)| t.iter().cloned()).collect();
    let rows = rows(app, session, account, &all).await;
    for (uri, tracks) in firsts {
        if !tracks.iter().any(|t| rows.unanswered.contains(t)) {
            made.insert(uri, covers::pick(&tracks, &rows.rows));
        }
    }
    library.with(|inner| {
        let kept = map_for(&mut inner.covers, account);
        kept.extend(made.iter().map(|(uri, urls)| (uri.clone(), urls.clone())));
    });
    found.extend(made);
    Ok(found)
}

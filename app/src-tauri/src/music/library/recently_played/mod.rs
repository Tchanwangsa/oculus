//! Recently played: the tracks the account played lately on any device,
//! newest first. Spotify's own Recents list (`list.rs`), its tracks only, is
//! merged by day with the tracks Oculus played (`recent_tracks/`), which
//! Spotify may never have heard of (`merge.rs`). Spotify's list failing
//! leaves Oculus's own. Spotify's tracks are kept per account until a
//! refresh, so a later call only merges Oculus's history in again.

mod list;
mod merge;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashSet};

use librespot_core::Session;
use tauri::AppHandle;

use super::error::LibraryError;
use super::search::PATHFINDER;
use super::spclient::{Method, Target};
use super::{call_to, logged, state};
use crate::music::recent_tracks::{self, Played, CAP};
use crate::runtime::blocking;

pub(super) use merge::Dated;

/// Pages of Spotify's list read at most: it mixes types, so a page can
/// hold few tracks.
const MAX_PAGES: usize = 4;

/// What one read of Spotify's list found.
#[derive(Debug, Default)]
struct Fetched {
    tracks: Vec<Dated>,
    /// Entries of any kind, nulls included.
    received: usize,
    /// Entries by type, for the log.
    types: BTreeMap<String, usize>,
    /// A later page failed; the earlier ones are kept.
    cut_short: bool,
}

/// Recently played for the connected account, Spotify's list from memory
/// unless `refresh` or the account changed. Fails only when nothing can be
/// shown: Spotify's list failed and Oculus played nothing.
pub(in crate::music) async fn recently_played(
    app: &AppHandle,
    refresh: bool,
) -> Result<Vec<Played>, LibraryError> {
    let session = state::session(app).ok_or(LibraryError::NotConnected)?;
    let account = session.username();
    let library = state::library(app);
    let local = {
        let account = account.clone();
        blocking::run(move || Ok(recent_tracks::read_blocking(&account)))
            .await
            .unwrap_or_default()
    };
    let kept = if refresh {
        library.with(|inner| inner.recently_played = None);
        None
    } else {
        library.with(|inner| match &inner.recently_played {
            Some((owner, tracks)) if *owner == account => Some(tracks.clone()),
            _ => None,
        })
    };
    let (spotify, fetched) = match kept {
        Some(tracks) => (Ok(tracks), None),
        None => match fetch(app, &session).await {
            Ok(fetched) => (Ok(fetched.tracks.clone()), Some(fetched)),
            Err(e) => (Err(e), None),
        },
    };
    let merged = merge::merge(spotify.as_deref().unwrap_or_default(), &local);
    if let Some(fetched) = fetched {
        let types: Vec<String> = fetched
            .types
            .iter()
            .map(|(kind, n)| format!("{kind} {n}"))
            .collect();
        elog!(
            "[music] recently played: {} items from Spotify{} ({} tracks; types: {}), {} local, {} shown",
            fetched.received,
            if fetched.cut_short { ", cut short" } else { "" },
            fetched.tracks.len(),
            types.join(", "),
            local.len(),
            merged.len()
        );
        library.with(|inner| inner.recently_played = Some((account.clone(), fetched.tracks)));
    }
    if merged.is_empty() {
        // Spotify's list failed and Oculus played nothing.
        spotify?;
    }
    Ok(merged)
}

/// Spotify's list from its start, `list::PAGE` entries a page, until it
/// has given `CAP` tracks, names no next page, or `MAX_PAGES` were read.
/// A failure, logged once here, fails the first page; on a later one what
/// earlier pages found is kept.
async fn fetch(app: &AppHandle, session: &Session) -> Result<Fetched, LibraryError> {
    let mut fetched = Fetched::default();
    let mut tracks: Vec<(String, Option<i64>)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut offset = 0;
    for read in 0..MAX_PAGES {
        let what = if read == 0 {
            "recently played, Oculus's own only"
        } else {
            "recently played, the rest of Spotify's list left out"
        };
        let page = call_to(
            app,
            session,
            what,
            Target::Url(PATHFINDER),
            Method::PostJson(list::body(offset)),
        )
        .await
        .and_then(|body| logged(what, list::parse_page(&body)));
        let page = match page {
            Ok(page) => page,
            Err(e) if read == 0 => return Err(e),
            Err(_) => {
                fetched.cut_short = true;
                break;
            }
        };
        let next = page.next(offset);
        fetched.received += page.received;
        for item in page.items {
            let kind = item.kind.clone().unwrap_or_else(|| "untyped".into());
            *fetched.types.entry(kind).or_default() += 1;
            if item.is_track() && seen.insert(item.uri.clone()) {
                tracks.push((item.uri, item.day));
            }
        }
        match next {
            Some(next) if tracks.len() < CAP => offset = next,
            _ => break,
        }
    }
    fetched.tracks = merge::dated(tracks);
    Ok(fetched)
}

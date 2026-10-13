//! Spotify's own library order for the account's playlists: pathfinder's
//! `libraryV3` persisted query (the request the web player's "Your Library"
//! sends), filtered to playlists and sorted by Recents, answered with
//! `{"data": {"me": {"libraryV3": {"items": [{"pinned", "item": {"_uri"}}],
//! "pagingInfo", "totalCount"}}}}`, pins first. `apply` puts the rootlist's
//! playlists in that order; anything it does not list keeps its rootlist
//! place after them. Like search's, the query is named by a hash that changes
//! with the web player (`docs/music-library.md`).

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::json;

use super::error::LibraryError;
use super::rootlist::{playlist_uri, Playlist};

/// The persisted-query hash of `libraryV3`, from the web player's bundle.
const LIBRARY_V3: &str = "390c78e5b951029bad359785e69b07b536a509c581cbcd0aded5e5067f187455";
/// Items asked for per request, as the web player asks.
pub(super) const PAGE: u32 = 50;

/// A playlist in Spotify's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Placed {
    pub(super) uri: String,
    pub(super) pinned: bool,
}

/// One page of the library.
#[derive(Debug, PartialEq)]
pub(super) struct Page {
    /// The playlists on it, in order; folders, Liked Songs and anything else
    /// are left out.
    pub(super) items: Vec<Placed>,
    /// Items on the page of any kind, nulls included.
    pub(super) received: usize,
    next_offset: Option<u32>,
    total: Option<u32>,
}

/// How the order matched the rootlist's playlists.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Applied {
    pub(super) matched: usize,
    pub(super) pinned: usize,
}

#[derive(Deserialize)]
struct Raw {
    data: Option<RawData>,
    errors: Option<Vec<Option<RawError>>>,
}

#[derive(Deserialize)]
struct RawError {
    message: Option<String>,
}

#[derive(Deserialize)]
struct RawData {
    me: Option<Me>,
}

#[derive(Deserialize)]
struct Me {
    #[serde(rename = "libraryV3")]
    library: Option<Library>,
}

/// A page, or an error typename such as `LibraryInvalidSortOrderIdError`.
#[derive(Deserialize)]
struct Library {
    #[serde(rename = "__typename")]
    typename: Option<String>,
    items: Option<Vec<Option<RawItem>>>,
    #[serde(rename = "pagingInfo")]
    paging: Option<Paging>,
    #[serde(rename = "totalCount")]
    total: Option<u32>,
}

#[derive(Deserialize)]
struct Paging {
    #[serde(rename = "nextOffset")]
    next_offset: Option<u32>,
}

#[derive(Deserialize)]
struct RawItem {
    pinned: Option<bool>,
    item: Option<Wrapped>,
}

#[derive(Deserialize)]
struct Wrapped {
    #[serde(rename = "_uri")]
    uri: Option<String>,
    data: Option<WrappedData>,
}

#[derive(Deserialize)]
struct WrappedData {
    uri: Option<String>,
}

/// The `libraryV3` request for the page at `offset`, with the variables the
/// web player sends.
pub(super) fn body(offset: u32) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "variables": {
            "filters": ["Playlists"],
            "order": "Recents",
            "textFilter": null,
            "features": ["LIKED_SONGS", "YOUR_EPISODES_V2", "PRERELEASES", "PRERELEASES_V2", "EVENTS"],
            "limit": PAGE,
            "offset": offset,
            "flatten": false,
            "expandedFolders": [],
            "folderUri": null,
            "includeFoldersWhenFlattening": true,
        },
        "operationName": "libraryV3",
        "extensions": {
            "persistedQuery": { "version": 1, "sha256Hash": LIBRARY_V3 },
        },
    }))
    .expect("a JSON value serialises")
}

/// One page of `libraryV3`. GraphQL errors arrive as a 200: a stale hash
/// (`PersistedQueryNotFound`), any other error without a page, or a page
/// typed as an error all fail it.
pub(super) fn parse_page(body: &[u8]) -> Result<Page, LibraryError> {
    let raw: Raw =
        serde_json::from_slice(body).map_err(|e| LibraryError::Unreadable(e.to_string()))?;
    let messages: Vec<String> = raw
        .errors
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .filter_map(|e| e.message)
        .collect();
    if messages.iter().any(|m| m == "PersistedQueryNotFound") {
        return Err(LibraryError::Unreadable(
            "PersistedQueryNotFound: libraryV3's hash needs replacing".into(),
        ));
    }
    let Some(library) = raw.data.and_then(|d| d.me).and_then(|m| m.library) else {
        return Err(LibraryError::Unreadable(if messages.is_empty() {
            "no libraryV3 in the answer".into()
        } else {
            messages.join("; ")
        }));
    };
    if let Some(typename) = library.typename.filter(|t| t.ends_with("Error")) {
        return Err(LibraryError::Unreadable(typename));
    }
    let raw_items = library.items.unwrap_or_default();
    let received = raw_items.len();
    let items = raw_items
        .into_iter()
        .flatten()
        .filter_map(|i| {
            let wrapped = i.item?;
            let uri = wrapped.uri.or_else(|| wrapped.data?.uri)?;
            Some(Placed {
                uri: playlist_uri(&uri)?,
                pinned: i.pinned.unwrap_or(false),
            })
        })
        .collect();
    Ok(Page {
        items,
        received,
        next_offset: library.paging.and_then(|p| p.next_offset),
        total: library.total,
    })
}

impl Page {
    /// The offset of the next page after this one at `offset`: pagingInfo's
    /// `nextOffset` when it names one, else past this page while
    /// `totalCount` says more follow (or, without it, while the page was
    /// full). None after an empty page.
    pub(super) fn next(&self, offset: u32) -> Option<u32> {
        if self.received == 0 {
            return None;
        }
        if let Some(next) = self.next_offset {
            return (next > offset).then_some(next);
        }
        let next = offset.saturating_add(u32::try_from(self.received).ok()?);
        match self.total {
            Some(total) => (next < total).then_some(next),
            None => (self.received >= PAGE as usize).then_some(next),
        }
    }
}

/// `list` in `order`'s order, each marked pinned as it is there; playlists
/// `order` does not list follow in their rootlist order.
pub(super) fn apply(mut list: Vec<Playlist>, order: &[Placed]) -> (Vec<Playlist>, Applied) {
    let mut rank: HashMap<&str, (usize, bool)> = HashMap::new();
    for (i, placed) in order.iter().enumerate() {
        rank.entry(placed.uri.as_str())
            .or_insert((i, placed.pinned));
    }
    let mut applied = Applied {
        matched: 0,
        pinned: 0,
    };
    for playlist in &mut list {
        let found = rank.get(playlist.uri.as_str());
        playlist.pinned = found.is_some_and(|(_, pinned)| *pinned);
        applied.matched += usize::from(found.is_some());
        applied.pinned += usize::from(playlist.pinned);
    }
    // Stable, so unmatched playlists keep their rootlist order.
    list.sort_by_key(|p| rank.get(p.uri.as_str()).map_or(usize::MAX, |(i, _)| *i));
    (list, applied)
}

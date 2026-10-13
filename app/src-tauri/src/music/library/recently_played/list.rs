//! Spotify's own Recents list: pathfinder's `recents` persisted query (the
//! request the web player's Recents page sends) for the list
//! `spotify:list:recents:page`, answered with `{"data": {"lists": [{
//! "__typename": "List", "items": {"pagingInfo": {"nextOffset"}, "items":
//! [{"entity": {"_uri", "data": {"entityTypeTrait": {"type"}}}, "addedAt":
//! {"year", "month", "day"}}]}}]}}`, newest first. It mixes tracks, albums,
//! playlists, artists and episodes, and dates each only to the day. Only the
//! URI, the type and the date are read; a track's row comes from its metadata
//! (`metadata.rs`). Like search's, the query is named by a hash that changes
//! with the web player (`docs/music-library.md`).

use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};

use super::super::error::LibraryError;
use crate::music::recent_tracks::is_track;

/// The persisted-query hash of `recents`, from the web player's bundle.
const RECENTS: &str = "698be5892a3cc95331deebeff463d05dfdd5febf5254bea30b895b5a93dfb584";
/// The list the web player's Recents page asks for.
const LIST: &str = "spotify:list:recents:page";
/// Items asked for per request.
pub(super) const PAGE: u32 = 50;
/// The entity type a track carries.
const TRACK: &str = "ENTITY_TYPE_TRACK";

/// One entry of the list, of any type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Item {
    pub(super) uri: String,
    /// `entityTypeTrait.type`, such as `ENTITY_TYPE_ALBUM`.
    pub(super) kind: Option<String>,
    /// Days since the Unix epoch of `addedAt`; none without a year.
    pub(super) day: Option<i64>,
}

impl Item {
    /// A `spotify:track:` URI, and a track's type when it names one.
    pub(super) fn is_track(&self) -> bool {
        is_track(&self.uri) && self.kind.as_deref().is_none_or(|k| k == TRACK)
    }
}

/// One page of the list.
#[derive(Debug, PartialEq)]
pub(super) struct Page {
    /// The entries with a URI, in order.
    pub(super) items: Vec<Item>,
    /// Entries on the page of any shape, nulls included.
    pub(super) received: usize,
    /// pagingInfo's `nextOffset`; none on the last page.
    next_offset: Option<u32>,
}

impl Page {
    /// The offset of the next page after this one at `offset`: pagingInfo's
    /// `nextOffset` when it is past `offset`; none after an empty page.
    pub(super) fn next(&self, offset: u32) -> Option<u32> {
        if self.received == 0 {
            return None;
        }
        self.next_offset.filter(|next| *next > offset)
    }
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
    lists: Option<Vec<Option<List>>>,
}

#[derive(Deserialize)]
struct List {
    #[serde(rename = "__typename")]
    typename: Option<String>,
    items: Option<Items>,
}

#[derive(Deserialize)]
struct Items {
    #[serde(rename = "pagingInfo")]
    paging: Option<Paging>,
    /// Read entry by entry, so one of an unexpected shape drops alone.
    items: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct Paging {
    #[serde(rename = "nextOffset")]
    next_offset: Option<u32>,
}

#[derive(Deserialize)]
struct RawItem {
    entity: Option<Entity>,
    #[serde(rename = "addedAt")]
    added_at: Option<Value>,
}

#[derive(Deserialize)]
struct Entity {
    #[serde(rename = "_uri")]
    uri: Option<String>,
    data: Option<EntityData>,
}

#[derive(Deserialize)]
struct EntityData {
    #[serde(rename = "entityTypeTrait")]
    type_trait: Option<TypeTrait>,
}

#[derive(Deserialize)]
struct TypeTrait {
    #[serde(rename = "type")]
    kind: Option<String>,
}

/// The `recents` request for the page at `offset`, with the variables the
/// web player sends.
pub(super) fn body(offset: u32) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "variables": { "uris": [LIST], "offset": offset, "limit": PAGE },
        "operationName": "recents",
        "extensions": {
            "persistedQuery": { "version": 1, "sha256Hash": RECENTS },
        },
    }))
    .expect("a JSON value serialises")
}

/// One page of `recents`. GraphQL errors arrive as a 200: a stale hash
/// (`PersistedQueryNotFound`), any other error without a list, or a first
/// list that is not a `List` (the web player's own check) all fail it.
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
            "PersistedQueryNotFound: recents' hash needs replacing".into(),
        ));
    }
    let Some(list) = raw
        .data
        .and_then(|d| d.lists)
        .and_then(|lists| lists.into_iter().next().flatten())
    else {
        return Err(LibraryError::Unreadable(if messages.is_empty() {
            "no list in the answer".into()
        } else {
            messages.join("; ")
        }));
    };
    match list.typename.as_deref() {
        Some("List") => {}
        other => {
            return Err(LibraryError::Unreadable(format!(
                "the recents list is a {}",
                other.unwrap_or("nameless type")
            )))
        }
    }
    let (paging, raw_items) = match list.items {
        Some(items) => (items.paging, items.items.unwrap_or_default()),
        None => (None, Vec::new()),
    };
    let received = raw_items.len();
    let items = raw_items
        .into_iter()
        .filter_map(|v| serde_json::from_value::<RawItem>(v).ok())
        .filter_map(|item| {
            let entity = item.entity?;
            let uri = entity.uri.filter(|u| !u.is_empty())?;
            Some(Item {
                uri,
                kind: entity.data.and_then(|d| d.type_trait).and_then(|t| t.kind),
                day: item.added_at.as_ref().and_then(day),
            })
        })
        .collect();
    Ok(Page {
        items,
        received,
        next_offset: paging.and_then(|p| p.next_offset),
    })
}

/// `{year, month, day}` as days since the Unix epoch; a missing month or
/// day is the first, a missing year or an impossible date none.
fn day(date: &Value) -> Option<i64> {
    let part = |key: &str| -> Option<i64> {
        match date.get(key)? {
            Value::Number(n) => n.as_i64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    };
    let year = i32::try_from(part("year")?).ok()?;
    let month = part("month").map_or(Some(1), |m| u32::try_from(m).ok())?;
    let day = part("day").map_or(Some(1), |d| u32::try_from(d).ok())?;
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)?;
    Some(date.signed_duration_since(epoch).num_days())
}

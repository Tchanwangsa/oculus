//! `GET /v1/me/playlists`, one page at a time.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::WebError;
use super::images::{self, Image};

/// Spotify's largest page.
pub(super) const PAGE: u32 = 50;
/// Enough for anyone's own list; past it, pages are calls the rate limit
/// would rather not see.
pub(super) const CAP: usize = 500;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Playlist {
    pub uri: String,
    pub name: String,
    pub owner: Option<String>,
    pub track_count: Option<u32>,
    pub image_url: Option<String>,
}

pub(super) struct Page {
    pub(super) items: Vec<Playlist>,
    pub(super) next: Option<String>,
}

#[derive(Deserialize)]
struct RawPage {
    #[serde(default)]
    items: Vec<Option<RawPlaylist>>,
    next: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawPlaylist {
    uri: Option<String>,
    name: Option<String>,
    owner: Option<RawOwner>,
    /// `{ total }`, named `tracks` or, in Spotify's newer shape, `items`.
    tracks: Option<Value>,
    items: Option<Value>,
    images: Option<Vec<Image>>,
}

#[derive(Deserialize)]
struct RawOwner {
    display_name: Option<String>,
    id: Option<String>,
}

pub(super) fn parse_page(body: &str) -> Result<Page, WebError> {
    let raw: RawPage =
        serde_json::from_str(body).map_err(|e| WebError::Unreadable(e.to_string()))?;
    Ok(Page {
        items: raw
            .items
            .into_iter()
            .flatten()
            .filter_map(from_raw)
            .collect(),
        next: raw.next,
    })
}

/// None for an entry without a usable URI or name (Spotify pads lists with
/// nulls and stubs).
pub(super) fn from_raw(raw: RawPlaylist) -> Option<Playlist> {
    let uri = raw.uri.filter(|u| u.starts_with("spotify:"))?;
    let name = raw.name.filter(|n| !n.trim().is_empty())?;
    let total = |v: &Option<Value>| {
        v.as_ref()?
            .get("total")?
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
    };
    Some(Playlist {
        uri,
        name,
        owner: raw
            .owner
            .and_then(|o| o.display_name.filter(|n| !n.is_empty()).or(o.id)),
        track_count: total(&raw.tracks).or_else(|| total(&raw.items)),
        image_url: images::pick(raw.images.as_deref()),
    })
}

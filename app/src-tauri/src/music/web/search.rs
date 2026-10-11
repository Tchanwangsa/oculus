//! `GET /v1/search` for playlists, albums and tracks, ten of each.

use serde::{Deserialize, Serialize};

use super::error::WebError;
use super::http::API;
use super::images::{self, Image};
use super::playlists::{self, RawPlaylist};

/// Spotify's largest search page.
const LIMIT: &str = "10";
/// A longer query is cut here.
pub(super) const MAX_QUERY: usize = 200;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SearchItem {
    pub uri: String,
    pub name: String,
    /// The owner, the artists, or the artists and album.
    pub subtitle: String,
    pub image_url: Option<String>,
    /// A track's album, so it plays on into the album.
    pub context_uri: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SearchResults {
    pub playlists: Vec<SearchItem>,
    pub albums: Vec<SearchItem>,
    pub tracks: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct Raw {
    playlists: Option<Group<RawPlaylist>>,
    albums: Option<Group<RawAlbum>>,
    tracks: Option<Group<RawTrack>>,
}

#[derive(Deserialize)]
struct Group<T> {
    #[serde(default = "Vec::new")]
    items: Vec<Option<T>>,
}

#[derive(Deserialize)]
struct RawAlbum {
    uri: Option<String>,
    name: Option<String>,
    #[serde(default)]
    artists: Vec<RawArtist>,
    images: Option<Vec<Image>>,
}

#[derive(Deserialize)]
struct RawTrack {
    uri: Option<String>,
    name: Option<String>,
    #[serde(default)]
    artists: Vec<RawArtist>,
    album: Option<RawAlbum>,
}

#[derive(Deserialize)]
struct RawArtist {
    name: Option<String>,
}

/// The query trimmed and cut to `MAX_QUERY` characters; empty asks nothing.
pub(super) fn clean(query: &str) -> Option<String> {
    let q: String = query.trim().chars().take(MAX_QUERY).collect();
    (!q.is_empty()).then_some(q)
}

pub(super) fn url(query: &str) -> String {
    url::Url::parse_with_params(
        &format!("{API}search"),
        [
            ("q", query),
            ("type", "playlist,album,track"),
            ("limit", LIMIT),
        ],
    )
    .map(String::from)
    .unwrap_or_else(|_| format!("{API}search"))
}

pub(super) fn parse(body: &str) -> Result<SearchResults, WebError> {
    let raw: Raw = serde_json::from_str(body).map_err(|e| WebError::Unreadable(e.to_string()))?;
    Ok(SearchResults {
        playlists: items(raw.playlists)
            .into_iter()
            .flatten()
            .filter_map(playlists::from_raw)
            .map(|p| SearchItem {
                uri: p.uri,
                name: p.name,
                subtitle: p.owner.unwrap_or_default(),
                image_url: p.image_url,
                context_uri: None,
            })
            .collect(),
        albums: items(raw.albums)
            .into_iter()
            .flatten()
            .filter_map(|a| {
                let (uri, name) = named(a.uri, a.name)?;
                Some(SearchItem {
                    uri,
                    name,
                    subtitle: artists(&a.artists),
                    image_url: images::pick(a.images.as_deref()),
                    context_uri: None,
                })
            })
            .collect(),
        tracks: items(raw.tracks)
            .into_iter()
            .flatten()
            .filter_map(|t| {
                let (uri, name) = named(t.uri, t.name)?;
                let album = t.album;
                let album_name = album.as_ref().and_then(|a| a.name.clone());
                let by = artists(&t.artists);
                Some(SearchItem {
                    uri,
                    name,
                    subtitle: match album_name.filter(|n| !n.is_empty()) {
                        Some(album) if !by.is_empty() => format!("{by} · {album}"),
                        Some(album) => album,
                        None => by,
                    },
                    image_url: album
                        .as_ref()
                        .and_then(|a| images::pick(a.images.as_deref())),
                    context_uri: album
                        .and_then(|a| a.uri)
                        .filter(|u| u.starts_with("spotify:album:")),
                })
            })
            .collect(),
    })
}

fn items<T>(group: Option<Group<T>>) -> Vec<Option<T>> {
    group.map(|g| g.items).unwrap_or_default()
}

fn named(uri: Option<String>, name: Option<String>) -> Option<(String, String)> {
    Some((
        uri.filter(|u| u.starts_with("spotify:"))?,
        name.filter(|n| !n.trim().is_empty())?,
    ))
}

fn artists(artists: &[RawArtist]) -> String {
    artists
        .iter()
        .filter_map(|a| a.name.as_deref())
        .filter(|n| !n.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

//! Search through Spotify's `searchview` service on the session's spclient
//! host: `GET /searchview/km/v4/search/<query>?entityVersion=2&limit=…`,
//! answered with `{"results": {"<type>": {"hits": [...], "total": n}}}`.
//! The request follows Spotify-Android-Good's librespot-based client
//! (`native/spotcore/src/catalog/search.rs`, `via_searchview`, which sends it
//! to spclient); the same route over Mercury is librespot-java's
//! `core/SearchManager.java`, and the response shape is librespot-golang's
//! `SearchResponse` (`librespot/metadata/metadata.go`).

use serde::{Deserialize, Serialize};

use super::error::LibraryError;
use super::rootlist::{encode, playlist_uri};

/// Hits asked for per type.
const LIMIT: u32 = 10;
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
    results: Option<RawResults>,
}

#[derive(Deserialize)]
struct RawResults {
    playlists: Option<Group<RawPlaylist>>,
    albums: Option<Group<RawAlbum>>,
    tracks: Option<Group<RawTrack>>,
}

#[derive(Deserialize)]
struct Group<T> {
    #[serde(default = "Vec::new")]
    hits: Vec<Option<T>>,
}

#[derive(Deserialize)]
struct RawPlaylist {
    uri: Option<String>,
    name: Option<String>,
    image: Option<String>,
    author: Option<String>,
}

#[derive(Deserialize)]
struct RawAlbum {
    uri: Option<String>,
    name: Option<String>,
    image: Option<String>,
    #[serde(default)]
    artists: Vec<Option<RawArtist>>,
}

#[derive(Deserialize)]
struct RawTrack {
    uri: Option<String>,
    name: Option<String>,
    /// The album's cover.
    image: Option<String>,
    #[serde(default)]
    artists: Vec<Option<RawArtist>>,
    album: Option<RawTrackAlbum>,
}

#[derive(Deserialize)]
struct RawTrackAlbum {
    uri: Option<String>,
    name: Option<String>,
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

pub(super) fn endpoint(query: &str, country: &str, username: &str) -> String {
    format!(
        "/searchview/km/v4/search/{}?limit={LIMIT}&entityVersion=2&imageSize=large&catalogue=premium&country={}&locale=en&username={}",
        encode(query),
        encode(country),
        encode(username)
    )
}

/// A status that means search itself is unavailable rather than failing for
/// this query: the route gone, moved or refused for this client.
pub(super) fn unavailable(e: LibraryError) -> LibraryError {
    match e {
        LibraryError::Forbidden => LibraryError::SearchUnavailable(403),
        LibraryError::Status(code @ (400 | 404 | 405 | 410 | 501)) => {
            LibraryError::SearchUnavailable(code)
        }
        e => e,
    }
}

pub(super) fn parse(body: &[u8]) -> Result<SearchResults, LibraryError> {
    let raw: Raw =
        serde_json::from_slice(body).map_err(|e| LibraryError::Unreadable(e.to_string()))?;
    let Some(results) = raw.results else {
        return Ok(SearchResults::default());
    };
    Ok(SearchResults {
        playlists: hits(results.playlists)
            .filter_map(|p| {
                let name = named(p.name)?;
                Some(SearchItem {
                    uri: playlist_uri(p.uri.as_deref()?)?,
                    name,
                    subtitle: match p.author.as_deref() {
                        Some("spotify") => "Spotify".into(),
                        other => other.unwrap_or_default().to_string(),
                    },
                    image_url: image(p.image),
                    context_uri: None,
                })
            })
            .collect(),
        albums: hits(results.albums)
            .filter_map(|a| {
                Some(SearchItem {
                    uri: uri_of(a.uri, "spotify:album:")?,
                    name: named(a.name)?,
                    subtitle: artists(&a.artists),
                    image_url: image(a.image),
                    context_uri: None,
                })
            })
            .collect(),
        tracks: hits(results.tracks)
            .filter_map(|t| {
                let uri = uri_of(t.uri, "spotify:track:")?;
                let name = named(t.name)?;
                let by = artists(&t.artists);
                let album = t.album;
                let album_name = album.as_ref().and_then(|a| a.name.clone());
                Some(SearchItem {
                    uri,
                    name,
                    subtitle: match album_name.filter(|n| !n.is_empty()) {
                        Some(album) if !by.is_empty() => format!("{by} · {album}"),
                        Some(album) => album,
                        None => by,
                    },
                    image_url: image(t.image),
                    context_uri: album.and_then(|a| uri_of(a.uri, "spotify:album:")),
                })
            })
            .collect(),
    })
}

fn hits<T>(group: Option<Group<T>>) -> impl Iterator<Item = T> {
    group
        .map(|g| g.hits)
        .unwrap_or_default()
        .into_iter()
        .flatten()
}

fn uri_of(uri: Option<String>, prefix: &str) -> Option<String> {
    uri.filter(|u| u.len() > prefix.len() && u.starts_with(prefix))
}

fn named(name: Option<String>) -> Option<String> {
    name.filter(|n| !n.trim().is_empty())
}

/// The hit's image as Spotify gives it, https only.
fn image(url: Option<String>) -> Option<String> {
    url.filter(|u| u.starts_with("https://"))
}

fn artists(artists: &[Option<RawArtist>]) -> String {
    artists
        .iter()
        .flatten()
        .filter_map(|a| a.name.as_deref())
        .filter(|n| !n.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

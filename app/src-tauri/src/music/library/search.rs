//! Search through pathfinder, the GraphQL service Spotify's web player and
//! desktop app search with: a POST of the `searchDesktop` persisted query to
//! `api-partner.spotify.com/pathfinder/v2/query`, answered with
//! `{"data": {"searchV2": {"tracksV2": …, "artists": …, "albumsV2": …,
//! "playlists": …}}}`.
//! More of one group is a page of its own operation (`searchTracks`,
//! `searchAlbums`, `searchPlaylists`), answered with that group alone and its
//! `pagingInfo.nextOffset`. Each query is named by the hash of its document,
//! which changes when Spotify ships a web player whose query differs;
//! pathfinder then answers 200 with a `PersistedQueryNotFound` error and the
//! hash needs replacing (`docs/music-library.md`).

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::error::LibraryError;
use super::rootlist::playlist_uri;

pub(super) const PATHFINDER: &str = "https://api-partner.spotify.com/pathfinder/v2/query";
/// The persisted-query hashes, from the web player's `xpui-routes-search`
/// chunk (checked 2026-10-11).
const SEARCH_DESKTOP: &str = "eef7cc54888d91bdd6802623477873caa3948ae173a0c34fd86827b267e94c03";
const SEARCH_TRACKS: &str = "b02683192a98dde7966b5e6655a79eeb62713eab703eda9902c932818dd52751";
const SEARCH_ALBUMS: &str = "202cb3305e31e5a0767ba7925f28bd728cf8f8b0217e6da43909056071cd70e9";
const SEARCH_PLAYLISTS: &str = "d520014e748f9ea44f7707d8df1819867ac1205e8b7f3e28f22fe5fc858921b1";
/// Hits asked for per type in the first answer.
const LIMIT: u32 = 10;
/// Hits asked for per page of one group.
const PAGE: u32 = 20;
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
    pub tracks: Vec<SearchItem>,
    pub artists: Vec<SearchItem>,
    pub albums: Vec<SearchItem>,
    pub playlists: Vec<SearchItem>,
}

/// A group that pages: one search filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchKind {
    Tracks,
    Albums,
    Playlists,
}

/// One page of a group, and the offset of the next if there is one.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SearchPage {
    pub items: Vec<SearchItem>,
    pub next_offset: Option<u32>,
}

#[derive(Deserialize)]
struct Raw {
    data: Option<RawData>,
    #[serde(default)]
    errors: Vec<RawError>,
}

#[derive(Deserialize)]
struct RawError {
    message: Option<String>,
}

#[derive(Deserialize)]
struct RawData {
    #[serde(rename = "searchV2")]
    search: Option<RawSearch>,
}

#[derive(Deserialize)]
struct RawSearch {
    playlists: Option<Group>,
    #[serde(rename = "albumsV2")]
    albums: Option<Group>,
    #[serde(rename = "tracksV2")]
    tracks: Option<Group>,
    artists: Option<Group>,
}

#[derive(Deserialize)]
struct Group {
    #[serde(default)]
    items: Vec<Option<Wrapper>>,
    #[serde(rename = "pagingInfo")]
    paging: Option<Paging>,
}

#[derive(Deserialize)]
struct Paging {
    #[serde(rename = "nextOffset")]
    next_offset: Option<u32>,
}

/// A hit, as `{"data": …}` or, for tracks, `{"item": {"data": …}}`.
#[derive(Deserialize)]
struct Wrapper {
    data: Option<Entity>,
    item: Option<Box<Wrapper>>,
}

impl Wrapper {
    fn entity(self) -> Option<Entity> {
        match self.data {
            Some(data) => Some(data),
            None => self.item?.entity(),
        }
    }
}

/// Whichever of a playlist's, artist's, album's or track's fields the hit
/// has; a `__typename` other than the group's own (`NotFound`,
/// `RestrictedContent`) is dropped.
#[derive(Deserialize)]
struct Entity {
    #[serde(rename = "__typename")]
    typename: Option<String>,
    uri: Option<String>,
    name: Option<String>,
    #[serde(rename = "ownerV2")]
    owner: Option<Owner>,
    images: Option<Images>,
    #[serde(rename = "coverArt")]
    cover_art: Option<Sources>,
    artists: Option<Artists>,
    #[serde(rename = "albumOfTrack")]
    album: Option<Box<Entity>>,
    profile: Option<Profile>,
    visuals: Option<Visuals>,
}

#[derive(Deserialize)]
struct Visuals {
    #[serde(rename = "avatarImage")]
    avatar: Option<Sources>,
}

#[derive(Deserialize)]
struct Owner {
    data: Option<OwnerData>,
}

#[derive(Deserialize)]
struct OwnerData {
    name: Option<String>,
}

#[derive(Deserialize)]
struct Images {
    #[serde(default)]
    items: Vec<Option<Sources>>,
}

#[derive(Deserialize)]
struct Sources {
    #[serde(default)]
    sources: Vec<Option<Source>>,
}

#[derive(Deserialize)]
struct Source {
    url: Option<String>,
    width: Option<u32>,
}

#[derive(Deserialize)]
struct Artists {
    #[serde(default)]
    items: Vec<Option<Artist>>,
}

#[derive(Deserialize)]
struct Artist {
    profile: Option<Profile>,
}

#[derive(Deserialize)]
struct Profile {
    name: Option<String>,
}

/// The query trimmed and cut to `MAX_QUERY` characters; empty asks nothing.
pub(super) fn clean(query: &str) -> Option<String> {
    let q: String = query.trim().chars().take(MAX_QUERY).collect();
    (!q.is_empty()).then_some(q)
}

/// The `searchDesktop` request body, with the variables the web player sends.
pub(super) fn body(query: &str) -> Vec<u8> {
    request("searchDesktop", SEARCH_DESKTOP, query, 0, LIMIT)
}

/// The request for the page of `kind` at `offset`.
pub(super) fn page_body(query: &str, kind: SearchKind, offset: u32) -> Vec<u8> {
    let (operation, hash) = match kind {
        SearchKind::Tracks => ("searchTracks", SEARCH_TRACKS),
        SearchKind::Albums => ("searchAlbums", SEARCH_ALBUMS),
        SearchKind::Playlists => ("searchPlaylists", SEARCH_PLAYLISTS),
    };
    request(operation, hash, query, offset, PAGE)
}

fn request(operation: &str, hash: &str, query: &str, offset: u32, limit: u32) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "variables": {
            "searchTerm": query,
            "offset": offset,
            "limit": limit,
            "numberOfTopResults": 5,
            "includeAudiobooks": false,
            "includeArtistHasConcertsField": false,
            "includePreReleases": false,
            "includeAlbumPreReleases": false,
            "includeAuthors": false,
            "includeEpisodeContentRatingsV2": false,
        },
        "operationName": operation,
        "extensions": {
            "persistedQuery": { "version": 1, "sha256Hash": hash },
        },
    }))
    .expect("a JSON value serialises")
}

/// A status that means search itself is unavailable rather than failing for
/// this query: the route gone, moved or refused for this client. A 412 is
/// pathfinder refusing a stale hash.
pub(super) fn unavailable(e: LibraryError) -> LibraryError {
    match e {
        LibraryError::Forbidden => LibraryError::SearchUnavailable(403),
        LibraryError::Status(412) => LibraryError::SearchOutdated,
        LibraryError::Status(code @ (400 | 404 | 405 | 410 | 501)) => {
            LibraryError::SearchUnavailable(code)
        }
        e => e,
    }
}

/// `searchV2`, once any GraphQL errors are ruled out; `None` when the answer
/// has none.
fn search_of(body: &[u8]) -> Result<Option<RawSearch>, LibraryError> {
    let raw: Raw =
        serde_json::from_slice(body).map_err(|e| LibraryError::Unreadable(e.to_string()))?;
    let messages: Vec<String> = raw.errors.into_iter().filter_map(|e| e.message).collect();
    if messages.iter().any(|m| m == "PersistedQueryNotFound") {
        return Err(LibraryError::SearchOutdated);
    }
    match raw.data.and_then(|d| d.search) {
        None if !messages.is_empty() => Err(LibraryError::Unreadable(messages.join("; "))),
        search => Ok(search),
    }
}

pub(super) fn parse(body: &[u8]) -> Result<SearchResults, LibraryError> {
    let Some(search) = search_of(body)? else {
        return Ok(SearchResults::default());
    };
    Ok(SearchResults {
        tracks: tracks(search.tracks),
        artists: artists(search.artists),
        albums: albums(search.albums),
        playlists: playlists(search.playlists),
    })
}

/// One page of `kind`; a page without `nextOffset` is the last.
pub(super) fn parse_page(body: &[u8], kind: SearchKind) -> Result<SearchPage, LibraryError> {
    let Some(search) = search_of(body)? else {
        return Ok(SearchPage::default());
    };
    let group = match kind {
        SearchKind::Tracks => search.tracks,
        SearchKind::Albums => search.albums,
        SearchKind::Playlists => search.playlists,
    };
    let next_offset = group
        .as_ref()
        .and_then(|g| g.paging.as_ref())
        .and_then(|p| p.next_offset);
    let items = match kind {
        SearchKind::Tracks => tracks(group),
        SearchKind::Albums => albums(group),
        SearchKind::Playlists => playlists(group),
    };
    Ok(SearchPage { items, next_offset })
}

fn playlists(group: Option<Group>) -> Vec<SearchItem> {
    hits(group, "Playlist")
        .filter_map(|p| {
            Some(SearchItem {
                uri: playlist_uri(p.uri.as_deref()?)?,
                name: named(p.name)?,
                subtitle: p
                    .owner
                    .and_then(|o| o.data)
                    .and_then(|d| d.name)
                    .unwrap_or_default(),
                image_url: p
                    .images
                    .and_then(|i| i.items.into_iter().flatten().next())
                    .and_then(image),
                context_uri: None,
            })
        })
        .collect()
}

fn artists(group: Option<Group>) -> Vec<SearchItem> {
    hits(group, "Artist")
        .filter_map(|a| {
            Some(SearchItem {
                uri: uri_of(a.uri, "spotify:artist:")?,
                name: named(a.profile?.name)?,
                subtitle: "Artist".into(),
                image_url: a.visuals.and_then(|v| v.avatar).and_then(image),
                context_uri: None,
            })
        })
        .collect()
}

fn albums(group: Option<Group>) -> Vec<SearchItem> {
    hits(group, "Album")
        .filter_map(|a| {
            Some(SearchItem {
                uri: uri_of(a.uri, "spotify:album:")?,
                name: named(a.name)?,
                subtitle: artist_names(a.artists),
                image_url: a.cover_art.and_then(image),
                context_uri: None,
            })
        })
        .collect()
}

fn tracks(group: Option<Group>) -> Vec<SearchItem> {
    hits(group, "Track")
        .filter_map(|t| {
            let uri = uri_of(t.uri, "spotify:track:")?;
            let name = named(t.name)?;
            let by = artist_names(t.artists);
            let (album_uri, album_name, cover) = match t.album.map(|a| *a) {
                Some(a) => (a.uri, a.name, a.cover_art),
                None => (None, None, None),
            };
            Some(SearchItem {
                uri,
                name,
                subtitle: match album_name.filter(|n| !n.is_empty()) {
                    Some(album) if !by.is_empty() => format!("{by} · {album}"),
                    Some(album) => album,
                    None => by,
                },
                image_url: cover.and_then(image),
                context_uri: uri_of(album_uri, "spotify:album:"),
            })
        })
        .collect()
}

fn hits(group: Option<Group>, typename: &'static str) -> impl Iterator<Item = Entity> {
    group
        .map(|g| g.items)
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .filter_map(Wrapper::entity)
        .filter(move |e| e.typename.as_deref().is_none_or(|t| t == typename))
}

fn uri_of(uri: Option<String>, prefix: &str) -> Option<String> {
    uri.filter(|u| u.len() > prefix.len() && u.starts_with(prefix))
}

fn named(name: Option<String>) -> Option<String> {
    name.filter(|n| !n.trim().is_empty())
}

/// The smallest https source at least 300 px wide, else the widest.
fn image(sources: Sources) -> Option<String> {
    let mut found: Vec<(u32, String)> = sources
        .sources
        .into_iter()
        .flatten()
        .filter_map(|s| Some((s.width.unwrap_or(0), s.url?)))
        .filter(|(_, url)| url.starts_with("https://"))
        .collect();
    found.sort_by_key(|(width, _)| *width);
    let pick = found
        .iter()
        .position(|(width, _)| *width >= 300)
        .unwrap_or(found.len().checked_sub(1)?);
    Some(found.swap_remove(pick).1)
}

fn artist_names(artists: Option<Artists>) -> String {
    artists
        .map(|a| a.items)
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .filter_map(|a| a.profile?.name)
        .filter(|n| !n.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

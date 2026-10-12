//! Track metadata in batches: `POST /extended-metadata/v0/extended-metadata`
//! with a `BatchedEntityRequest` asking `TRACK_V4` for each track, the
//! request librespot's `SpClient::get_extended_metadata` sends (and
//! `get_track_metadata` fills for one track). Each answered entity carries a
//! `metadata::Track` in its `extension_data`. The same request asking
//! `ALBUM_V4` for an album answers a `metadata::Album`, whose discs list its
//! tracks' gids.

use std::collections::HashMap;

use librespot_core::SpotifyId;
use librespot_protocol::extended_metadata::{
    BatchedEntityRequest, BatchedExtensionResponse, EntityRequest, ExtensionQuery,
};
use librespot_protocol::extension_kind::ExtensionKind;
use librespot_protocol::metadata::{image::Size, Album, Image, Track};
use protobuf::{EnumOrUnknown, Message};
use serde::Serialize;

use super::error::LibraryError;

pub(super) const ENDPOINT: &str = "/extended-metadata/v0/extended-metadata";
/// Tracks asked for per request.
pub(super) const BATCH: usize = 100;
const IMAGE_BASE: &str = "https://i.scdn.co/image/";

/// What a track row shows.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TrackRow {
    pub uri: String,
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    /// The album's cover at its `DEFAULT` size, else its largest.
    pub image_url: Option<String>,
    pub duration_ms: u32,
    /// Tells albums apart: the album's gid in hex, else its cover's URL,
    /// else its name.
    #[serde(skip)]
    pub(in crate::music) album_key: String,
}

/// The `BatchedEntityRequest` asking `kind` for each of these entities.
pub(super) fn request(uris: &[String], kind: ExtensionKind) -> Vec<u8> {
    BatchedEntityRequest {
        entity_request: uris
            .iter()
            .map(|uri| EntityRequest {
                entity_uri: uri.clone(),
                query: vec![ExtensionQuery {
                    extension_kind: EnumOrUnknown::new(kind),
                    ..ExtensionQuery::default()
                }],
                ..EntityRequest::default()
            })
            .collect(),
        ..BatchedEntityRequest::default()
    }
    .write_to_bytes()
    .unwrap_or_default()
}

/// Each answered track's row, by URI. An entity with no data, or data that
/// is not a named track, is left out.
pub(super) fn parse(body: &[u8]) -> Result<HashMap<String, TrackRow>, LibraryError> {
    let response = BatchedExtensionResponse::parse_from_bytes(body)
        .map_err(|e| LibraryError::Unreadable(e.to_string()))?;
    Ok(response
        .extended_metadata
        .iter()
        .flat_map(|array| array.extension_data.iter())
        .filter_map(|data| {
            let any = data.extension_data.as_ref()?;
            let track = Track::parse_from_bytes(&any.value).ok()?;
            let row = row(&data.entity_uri, &track)?;
            Some((data.entity_uri.clone(), row))
        })
        .collect())
}

/// The album's tracks as URIs, disc by disc in order; a track without a gid
/// is left out, and an answer with no album is empty.
pub(super) fn album_tracks(body: &[u8]) -> Result<Vec<String>, LibraryError> {
    let response = BatchedExtensionResponse::parse_from_bytes(body)
        .map_err(|e| LibraryError::Unreadable(e.to_string()))?;
    let Some(album) = response
        .extended_metadata
        .iter()
        .flat_map(|array| array.extension_data.iter())
        .find_map(|data| Album::parse_from_bytes(&data.extension_data.as_ref()?.value).ok())
    else {
        return Ok(Vec::new());
    };
    Ok(album
        .disc
        .iter()
        .flat_map(|disc| disc.track.iter())
        .filter_map(|track| SpotifyId::from_raw(track.gid()).ok())
        .map(|id| format!("spotify:track:{}", id.to_base62()))
        .collect())
}

pub(super) fn row(uri: &str, track: &Track) -> Option<TrackRow> {
    let name = track.name().trim();
    if name.is_empty() {
        return None;
    }
    let album = track.album.as_ref();
    let image_url = album.and_then(cover);
    let album_name = album.map(|a| a.name().to_string()).unwrap_or_default();
    let album_key = match album.map(|a| a.gid()).filter(|g| !g.is_empty()) {
        Some(gid) => hex(gid),
        None => image_url.clone().unwrap_or_else(|| album_name.clone()),
    };
    Some(TrackRow {
        uri: uri.to_string(),
        name: name.to_string(),
        artists: track
            .artist
            .iter()
            .map(|a| a.name().to_string())
            .filter(|n| !n.is_empty())
            .collect(),
        album: album_name,
        image_url,
        duration_ms: u32::try_from(track.duration()).unwrap_or(0),
        album_key,
    })
}

/// librespot reads `cover_group`; `cover` is the older list of the same.
fn cover(album: &Album) -> Option<String> {
    if album.cover_group.image.is_empty() {
        image_url(&album.cover)
    } else {
        image_url(&album.cover_group.image)
    }
}

/// The `DEFAULT` size (300 px, plenty for a row's thumbnail), else the
/// largest listed.
fn image_url(images: &[Image]) -> Option<String> {
    let with_id: Vec<&Image> = images.iter().filter(|i| !i.file_id().is_empty()).collect();
    let rank = |size: Size| match size {
        Size::SMALL => 0,
        Size::DEFAULT => 1,
        Size::LARGE => 2,
        Size::XLARGE => 3,
    };
    let chosen = with_id
        .iter()
        .find(|i| i.size() == Size::DEFAULT)
        .or_else(|| with_id.iter().max_by_key(|i| rank(i.size())))?;
    Some(format!("{IMAGE_BASE}{}", hex(chosen.file_id())))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

//! A librespot `AudioItem` as the snapshot's `Track`, with its cover on
//! Spotify's image CDN.

use librespot_metadata::audio::{AudioItem, UniqueFields};

use super::state::Track;

const COVER_BASE: &str = "https://i.scdn.co/image/";
/// The cover size asked for: the largest at most this wide (Spotify lists
/// 64, 300 and 640), else the smallest there is.
const COVER_WIDTH: i32 = 640;

pub(super) fn from_item(item: &AudioItem) -> Track {
    let (artists, album) = match &item.unique_fields {
        UniqueFields::Track { artists, album, .. } => (
            artists.iter().map(|a| a.name.clone()).collect(),
            album.clone(),
        ),
        UniqueFields::Episode { show_name, .. } => (vec![show_name.clone()], show_name.clone()),
        UniqueFields::Local { artists, album, .. } => (
            artists.iter().cloned().collect(),
            album.clone().unwrap_or_default(),
        ),
    };
    Track {
        uri: item.uri.clone(),
        name: item.name.clone(),
        artists,
        album,
        cover_url: cover_url(item.covers.iter().map(|c| (c.url.as_str(), c.width))),
        duration_ms: item.duration_ms,
    }
}

/// `https://i.scdn.co/image/<hex id>` for the chosen cover. librespot builds
/// each URL from an account attribute that may name another host, so only
/// the file id is kept from it.
pub(super) fn cover_url<'a>(covers: impl Iterator<Item = (&'a str, i32)>) -> Option<String> {
    let mut covers: Vec<(&str, i32)> = covers.filter(|(url, _)| file_id(url).is_some()).collect();
    covers.sort_by_key(|(_, width)| *width);
    let chosen = covers
        .iter()
        .rev()
        .find(|(_, width)| *width <= COVER_WIDTH)
        .or(covers.first())?;
    file_id(chosen.0).map(|id| format!("{COVER_BASE}{id}"))
}

fn file_id(url: &str) -> Option<&str> {
    let id = url.rsplit('/').next()?;
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit())).then_some(id)
}

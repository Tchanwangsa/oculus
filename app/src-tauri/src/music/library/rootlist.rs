//! The account's playlists from the rootlist: `GET
//! /playlist/v2/user/<user>/rootlist`, the request librespot's
//! `SpClient::get_rootlist` makes, a playlist4 `SelectedListContent` whose
//! meta items (when there is one per item) carry each playlist's name,
//! owner, length and picture. Folders are flattened: their start and end
//! markers are skipped and the playlists inside keep their place.

use librespot_protocol::playlist4_external::{ListAttributes, SelectedListContent};
use protobuf::Message;
use serde::Serialize;

use super::error::LibraryError;

/// Items asked for per rootlist request.
pub(super) const PAGE: usize = 500;
/// Enough for anyone's own list.
pub(super) const CAP: usize = 500;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Playlist {
    pub uri: String,
    pub name: String,
    pub owner: Option<String>,
    pub track_count: Option<u32>,
    /// The cover: one URL, or four for a 2×2 grid; none until one is known
    /// (`covers.rs` makes one for a playlist Spotify gives none).
    pub images: Vec<String>,
    /// Pinned to the top of the account's library in Spotify (`order.rs`).
    pub pinned: bool,
}

/// What a row shows of a playlist, from its rootlist decorations or, when
/// those are missing, its own `/playlist/v2/playlist/<id>`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Header {
    pub(super) name: String,
    pub(super) owner: Option<String>,
    pub(super) track_count: Option<u32>,
    pub(super) image_url: Option<String>,
}

impl Header {
    pub(super) fn playlist(self, uri: String) -> Playlist {
        Playlist {
            uri,
            name: self.name,
            owner: self.owner,
            track_count: self.track_count,
            images: self.image_url.into_iter().collect(),
            pinned: false,
        }
    }
}

/// One playlist in the rootlist; `header` is none when the rootlist carried
/// no name for it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Entry {
    pub(super) uri: String,
    pub(super) header: Option<Header>,
}

pub(super) struct Page {
    pub(super) entries: Vec<Entry>,
    /// Items on the page, folder markers included: the next page's `from`.
    pub(super) items: usize,
    /// More items follow.
    pub(super) truncated: bool,
}

pub(super) fn endpoint(user: &str, from: usize) -> String {
    format!(
        "/playlist/v2/user/{}/rootlist?decorate=revision,attributes,length,owner,capabilities,status_code&from={from}&length={PAGE}",
        encode(user)
    )
}

pub(super) fn playlist_endpoint(uri: &str) -> Option<String> {
    let id = uri.strip_prefix("spotify:playlist:")?;
    Some(format!("/playlist/v2/playlist/{}", encode(id)))
}

pub(super) fn parse_page(body: &[u8]) -> Result<Page, LibraryError> {
    let list = decode(body)?;
    let contents = &list.contents;
    // Decorations are index-aligned with the items when there is one each.
    let aligned = contents.meta_items.len() == contents.items.len();
    let entries = contents
        .items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let uri = playlist_uri(item.uri())?;
            let meta = aligned.then(|| &contents.meta_items[i]);
            // A playlist its owner deleted stays in the rootlist as a 404.
            if meta.is_some_and(|m| matches!(m.status_code, Some(404 | 410))) {
                return None;
            }
            let header =
                meta.and_then(|m| header(m.attributes.as_ref()?, m.owner_username(), m.length));
            Some(Entry { uri, header })
        })
        .collect();
    Ok(Page {
        entries,
        items: contents.items.len(),
        truncated: contents.truncated(),
    })
}

/// A playlist's own header, from `/playlist/v2/playlist/<id>`.
pub(super) fn parse_playlist(body: &[u8]) -> Result<Option<Header>, LibraryError> {
    let list = decode(body)?;
    Ok(list
        .attributes
        .as_ref()
        .and_then(|a| header(a, list.owner_username(), list.length)))
}

fn decode(body: &[u8]) -> Result<SelectedListContent, LibraryError> {
    SelectedListContent::parse_from_bytes(body).map_err(|e| LibraryError::Unreadable(e.to_string()))
}

/// `spotify:playlist:<id>`, from that or the older
/// `spotify:user:<user>:playlist:<id>`; none for a folder marker
/// (`spotify:start-group:…`, `spotify:end-group:…`) or anything else.
pub(super) fn playlist_uri(uri: &str) -> Option<String> {
    let id = match uri.strip_prefix("spotify:playlist:") {
        Some(id) => id,
        None => {
            let rest = uri.strip_prefix("spotify:user:")?;
            rest.split_once(":playlist:")?.1
        }
    };
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric()))
        .then(|| format!("spotify:playlist:{id}"))
}

fn header(attrs: &ListAttributes, owner: &str, length: Option<i32>) -> Option<Header> {
    let name = attrs.name().trim();
    if name.is_empty() {
        return None;
    }
    Some(Header {
        name: name.to_string(),
        owner: match owner {
            "" => None,
            "spotify" => Some("Spotify".into()),
            other => Some(other.to_string()),
        },
        track_count: length.and_then(|n| u32::try_from(n).ok()),
        image_url: image(attrs),
    })
}

/// The custom picture as `https://i.scdn.co/image/<hex>`; without one, the
/// generated cover Spotify links (a four-cover grid on `mosaic.scdn.co`),
/// its `default` size first; none when neither is there.
fn image(attrs: &ListAttributes) -> Option<String> {
    let picture = attrs.picture();
    if !picture.is_empty() {
        let hex: String = picture.iter().map(|b| format!("{b:02x}")).collect();
        return Some(format!("https://i.scdn.co/image/{hex}"));
    }
    let on_cdn = |url: &str| {
        url::Url::parse(url).is_ok_and(|u| {
            u.scheme() == "https" && u.host_str().is_some_and(|h| h.ends_with(".scdn.co"))
        })
    };
    let sizes: Vec<_> = attrs
        .picture_size
        .iter()
        .filter(|p| on_cdn(p.url()))
        .collect();
    sizes
        .iter()
        .find(|p| p.target_name() == "default")
        .or(sizes.first())
        .map(|p| p.url().to_string())
}

/// A path segment, percent-encoded.
pub(super) fn encode(segment: &str) -> String {
    url::form_urlencoded::byte_serialize(segment.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

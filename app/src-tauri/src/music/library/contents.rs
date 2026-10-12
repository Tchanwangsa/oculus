//! A playlist's tracks from its own `GET /playlist/v2/playlist/<id>` (the
//! request librespot's `SpClient::get_playlist` makes): a playlist4
//! `SelectedListContent` whose `contents.items` are the entries in order.
//! The plain request is sent first. librespot pages only the rootlist, with
//! `from`/`length`; when a playlist's answer says it is `truncated`, the rest
//! is asked for the same way, and a page that does not start where it was
//! asked to (the parameters ignored) ends the list there.

use librespot_protocol::playlist4_external::SelectedListContent;
use protobuf::Message;

use super::error::LibraryError;
use super::rootlist::playlist_endpoint;

/// Entries asked for per follow-up page, as for the rootlist.
pub(super) const PAGE: usize = 500;
/// Spotify's own limit on a playlist's length.
pub(super) const CAP: usize = 10_000;

/// A playlist's tracks as far as they have been read.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Contents {
    pub(super) tracks: Vec<String>,
    /// Where the next page starts, while there is more to read.
    pub(super) next: Option<usize>,
}

/// One answer: its catalogue tracks, and where it sits in the playlist.
#[derive(Debug, PartialEq)]
pub(super) struct Page {
    pub(super) tracks: Vec<String>,
    /// The first entry's position.
    pub(super) pos: usize,
    /// Entries on the page, episodes and local files included.
    pub(super) items: usize,
    pub(super) truncated: bool,
}

/// The page starting at entry `from`; the first is `playlist_endpoint`
/// alone.
pub(super) fn page_endpoint(uri: &str, from: usize) -> Option<String> {
    Some(format!(
        "{}?from={from}&length={PAGE}",
        playlist_endpoint(uri)?
    ))
}

pub(super) fn parse_page(body: &[u8]) -> Result<Page, LibraryError> {
    let list = SelectedListContent::parse_from_bytes(body)
        .map_err(|e| LibraryError::Unreadable(e.to_string()))?;
    let contents = &list.contents;
    Ok(Page {
        tracks: contents
            .items
            .iter()
            .map(|item| item.uri())
            .filter(|uri| is_track(uri))
            .map(str::to_string)
            .collect(),
        pos: usize::try_from(contents.pos()).unwrap_or(0),
        items: contents.items.len(),
        truncated: contents.truncated(),
    })
}

impl Contents {
    /// After the first page.
    pub(super) fn first(page: Page) -> Self {
        let next = page.truncated.then_some(page.pos + page.items);
        Self {
            tracks: page.tracks,
            next,
        }
    }

    /// Adds the page asked for at `from`. One that starts elsewhere or is
    /// empty ends the list, as does reaching `CAP` tracks.
    pub(super) fn extend(&mut self, from: usize, page: Page) {
        if page.pos != from || page.items == 0 {
            self.next = None;
            return;
        }
        self.tracks.extend(page.tracks);
        self.tracks.truncate(CAP);
        self.next = (page.truncated && self.tracks.len() < CAP).then_some(from + page.items);
    }
}

/// A catalogue track: local files (`spotify:local:…`), episodes and anything
/// else are not.
pub(super) fn is_track(uri: &str) -> bool {
    uri.strip_prefix("spotify:track:")
        .is_some_and(|id| id.len() == 22 && id.bytes().all(|b| b.is_ascii_alphanumeric()))
}

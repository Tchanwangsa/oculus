use std::time::{Duration, Instant};

use librespot_protocol::playlist4_external::{
    Item, ListAttributes, ListItems, MetaItem, PictureSize, SelectedListContent,
};
use protobuf::Message;

use super::error::LibraryError;
use super::gate::{retry_after, Gate, DEFAULT_WAIT};
use super::rootlist::{
    self, encode, parse_page, parse_playlist, playlist_endpoint, playlist_uri, Entry, Header,
};
use super::search::{self, clean, parse, MAX_QUERY};
use super::spclient::status_error;

fn item(uri: &str) -> Item {
    Item {
        uri: Some(uri.into()),
        ..Item::default()
    }
}

fn attributes(name: &str, picture: &[u8]) -> ListAttributes {
    ListAttributes {
        name: Some(name.into()),
        picture: (!picture.is_empty()).then(|| picture.to_vec()),
        ..ListAttributes::default()
    }
}

fn meta(attrs: Option<ListAttributes>, owner: &str, length: i32) -> MetaItem {
    MetaItem {
        attributes: attrs.into(),
        owner_username: Some(owner.into()),
        length: Some(length),
        ..MetaItem::default()
    }
}

/// A rootlist in the shape `get_rootlist` returns: a folder around one
/// playlist, an older user-form URI, a mosaic-only cover, a playlist its
/// owner deleted, and one meta item per item.
fn rootlist_bytes() -> Vec<u8> {
    let mut mosaic = attributes("Study mix", &[]);
    mosaic.picture_size = vec![
        PictureSize {
            target_name: Some("large".into()),
            url: Some("https://mosaic.scdn.co/640/aa".into()),
            ..PictureSize::default()
        },
        PictureSize {
            target_name: Some("default".into()),
            url: Some("https://mosaic.scdn.co/300/aa".into()),
            ..PictureSize::default()
        },
    ];
    let mut gone = meta(Some(attributes("Gone", &[])), "tanat", 3);
    gone.status_code = Some(404);
    let contents = ListItems {
        pos: Some(0),
        truncated: Some(false),
        items: vec![
            item("spotify:start-group:abc:Uni"),
            item("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M"),
            item("spotify:end-group:abc"),
            item("spotify:user:tanat:playlist:1a2b"),
            item("spotify:playlist:3c4d"),
            item("spotify:playlist:dead"),
        ],
        meta_items: vec![
            meta(None, "", 0),
            meta(
                Some(attributes("Focus", &[0xab, 0x67, 0x01])),
                "spotify",
                91,
            ),
            meta(None, "", 0),
            meta(Some(mosaic), "tanat", 12),
            // No decorations: its name comes from the playlist itself.
            MetaItem::default(),
            gone,
        ],
        ..ListItems::default()
    };
    SelectedListContent {
        length: Some(6),
        contents: Some(contents).into(),
        ..SelectedListContent::default()
    }
    .write_to_bytes()
    .unwrap()
}

#[test]
fn the_rootlist_flattens_folders_and_reads_each_playlists_meta_item() {
    let page = parse_page(&rootlist_bytes()).unwrap();
    assert_eq!(page.items, 6);
    assert!(!page.truncated);
    assert_eq!(
        page.entries,
        [
            Entry {
                uri: "spotify:playlist:37i9dQZF1DXcBWIGoYBM5M".into(),
                header: Some(Header {
                    name: "Focus".into(),
                    owner: Some("Spotify".into()),
                    track_count: Some(91),
                    image_url: Some("https://i.scdn.co/image/ab6701".into()),
                }),
            },
            Entry {
                uri: "spotify:playlist:1a2b".into(),
                header: Some(Header {
                    name: "Study mix".into(),
                    owner: Some("tanat".into()),
                    track_count: Some(12),
                    image_url: Some("https://mosaic.scdn.co/300/aa".into()),
                }),
            },
            Entry {
                uri: "spotify:playlist:3c4d".into(),
                header: None,
            },
        ]
    );
}

#[test]
fn meta_items_that_do_not_line_up_are_ignored() {
    let list = SelectedListContent {
        contents: Some(ListItems {
            items: vec![item("spotify:playlist:a1"), item("spotify:playlist:b2")],
            meta_items: vec![meta(Some(attributes("Only one", &[])), "x", 1)],
            pos: Some(0),
            truncated: Some(true),
            ..ListItems::default()
        })
        .into(),
        ..SelectedListContent::default()
    };
    let page = parse_page(&list.write_to_bytes().unwrap()).unwrap();
    assert!(page.truncated);
    assert!(page.entries.iter().all(|e| e.header.is_none()));
    assert!(matches!(
        parse_page(b"\xff\xff\xff"),
        Err(LibraryError::Unreadable(_))
    ));
}

#[test]
fn a_playlists_own_header_names_it() {
    let list = SelectedListContent {
        attributes: Some(attributes("Late night", &[0x0f])).into(),
        owner_username: Some("tanat".into()),
        length: Some(40),
        ..SelectedListContent::default()
    };
    let header = parse_playlist(&list.write_to_bytes().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(header.name, "Late night");
    assert_eq!(header.track_count, Some(40));
    assert_eq!(
        header.image_url.as_deref(),
        Some("https://i.scdn.co/image/0f")
    );
    let unnamed = SelectedListContent::default().write_to_bytes().unwrap();
    assert_eq!(parse_playlist(&unnamed).unwrap(), None);
}

#[test]
fn rootlist_endpoints_and_uris() {
    assert_eq!(
        rootlist::endpoint("a b", 500),
        "/playlist/v2/user/a%20b/rootlist?decorate=revision,attributes,length,owner,capabilities,status_code&from=500&length=500"
    );
    assert_eq!(
        playlist_endpoint("spotify:playlist:3c4d").as_deref(),
        Some("/playlist/v2/playlist/3c4d")
    );
    assert_eq!(playlist_uri("spotify:start-group:abc:Uni"), None);
    assert_eq!(playlist_uri("spotify:playlist:"), None);
    assert_eq!(playlist_uri("spotify:collection"), None);
    assert_eq!(encode("é/+?"), "%C3%A9%2F%2B%3F");
}

/// A `searchview/km/v4` answer in the shape librespot-golang's
/// `SearchResponse` describes, trimmed: a null hit, a hit without a URI, a
/// playlist under the older user-form URI, a plain-http image.
const SEARCH: &str = r#"{
  "results": {
    "tracks": { "hits": [
      { "name": "So What", "uri": "spotify:track:tr1",
        "image": "https://i.scdn.co/image/a300", "duration": 562000,
        "artists": [{ "name": "Miles Davis", "uri": "spotify:artist:ar1" },
                    { "name": "John Coltrane", "uri": "spotify:artist:ar2" }],
        "album": { "name": "Kind of Blue", "uri": "spotify:album:al1" } },
      null,
      { "name": "Loose", "uri": "spotify:track:tr2", "artists": [] },
      { "name": "No uri" }
    ], "total": 800 },
    "albums": { "hits": [
      { "name": "Kind of Blue", "uri": "spotify:album:al1",
        "image": "http://insecure.example/a",
        "artists": [{ "name": "Miles Davis", "uri": "spotify:artist:ar1" }] }
    ], "total": 1 },
    "playlists": { "hits": [
      { "name": "Jazz Classics", "uri": "spotify:user:spotify:playlist:pl1",
        "image": "https://i.scdn.co/image/p1", "followersCount": 1000, "author": "spotify" },
      { "name": "", "uri": "spotify:playlist:pl2" }
    ], "total": 2 },
    "artists": { "hits": [], "total": 0 }
  },
  "requestId": "abc",
  "categoriesOrder": ["topHit", "tracks", "artists", "albums", "playlists"]
}"#;

#[test]
fn search_hits_group_with_subtitles_and_a_tracks_album() {
    let r = parse(SEARCH.as_bytes()).unwrap();
    assert_eq!(r.playlists.len(), 1);
    assert_eq!(r.playlists[0].uri, "spotify:playlist:pl1");
    assert_eq!(r.playlists[0].subtitle, "Spotify");
    assert_eq!(
        r.playlists[0].image_url.as_deref(),
        Some("https://i.scdn.co/image/p1")
    );
    assert_eq!(r.albums[0].subtitle, "Miles Davis");
    assert_eq!(r.albums[0].image_url, None);
    assert_eq!(r.tracks.len(), 2);
    assert_eq!(
        r.tracks[0].subtitle,
        "Miles Davis, John Coltrane · Kind of Blue"
    );
    assert_eq!(
        r.tracks[0].context_uri.as_deref(),
        Some("spotify:album:al1")
    );
    assert_eq!(
        r.tracks[0].image_url.as_deref(),
        Some("https://i.scdn.co/image/a300")
    );
    assert_eq!(r.tracks[1].subtitle, "");
    assert_eq!(r.tracks[1].context_uri, None);
    // No results, or a group left out, is empty; junk is unreadable.
    assert_eq!(parse(b"{}").unwrap(), Default::default());
    assert_eq!(parse(br#"{"results":{}}"#).unwrap(), Default::default());
    assert!(matches!(parse(b"<html>"), Err(LibraryError::Unreadable(_))));
}

#[test]
fn a_search_query_is_trimmed_capped_and_encoded_into_the_path() {
    assert_eq!(clean("   "), None);
    assert_eq!(clean("  lo-fi ").as_deref(), Some("lo-fi"));
    assert_eq!(clean(&"é".repeat(300)).unwrap().chars().count(), MAX_QUERY);
    assert_eq!(
        search::endpoint("kind of blue & more", "TH", "tanat"),
        "/searchview/km/v4/search/kind%20of%20blue%20%26%20more?limit=10&entityVersion=2&imageSize=large&catalogue=premium&country=TH&locale=en&username=tanat"
    );
}

#[test]
fn a_missing_search_route_reads_as_search_unavailable() {
    assert_eq!(
        search::unavailable(LibraryError::Status(404)),
        LibraryError::SearchUnavailable(404)
    );
    assert_eq!(
        search::unavailable(LibraryError::Forbidden),
        LibraryError::SearchUnavailable(403)
    );
    assert_eq!(
        search::unavailable(LibraryError::RateLimited(9)),
        LibraryError::RateLimited(9)
    );
    assert_eq!(
        search::unavailable(LibraryError::Status(502)),
        LibraryError::Status(502)
    );
    assert_eq!(
        LibraryError::SearchUnavailable(404).to_string(),
        "Search isn't available right now (Spotify answered 404)"
    );
}

#[test]
fn a_429_closes_the_gate_until_retry_after() {
    let t0 = Instant::now();
    let mut gate = Gate::default();
    assert_eq!(gate.closed_for(t0), None);
    gate.close(t0, retry_after(Some("7")));
    assert_eq!(gate.closed_for(t0), Some(7));
    assert_eq!(gate.closed_for(t0 + Duration::from_millis(1500)), Some(6));
    // A shorter wait never shortens the gate.
    gate.close(t0, Duration::from_secs(2));
    assert_eq!(gate.closed_for(t0 + Duration::from_secs(3)), Some(4));
    assert_eq!(gate.closed_for(t0 + Duration::from_secs(7)), None);
    // Open again for good.
    assert_eq!(gate.closed_for(t0), None);
}

#[test]
fn retry_after_falls_back_when_missing_or_a_date() {
    assert_eq!(retry_after(Some(" 12 ")), Duration::from_secs(12));
    assert_eq!(retry_after(None), DEFAULT_WAIT);
    assert_eq!(
        retry_after(Some("Wed, 21 Oct 2026 07:28:00 GMT")),
        DEFAULT_WAIT
    );
}

#[test]
fn statuses_read_as_typed_errors() {
    assert_eq!(status_error(401, None), LibraryError::Unauthorized);
    assert_eq!(status_error(403, None), LibraryError::Forbidden);
    assert_eq!(status_error(429, Some("40")), LibraryError::RateLimited(40));
    assert_eq!(status_error(502, None), LibraryError::Status(502));
    assert_eq!(
        LibraryError::RateLimited(40).to_string(),
        "Spotify is rate-limiting Oculus; try again in 40 s"
    );
    assert_eq!(
        LibraryError::NotConnected.to_string(),
        "Not connected to Spotify"
    );
}

use std::collections::HashMap;
use std::time::{Duration, Instant};

use librespot_core::SpotifyId;
use librespot_protocol::entity_extension_data::EntityExtensionData;
use librespot_protocol::extended_metadata::{
    BatchedEntityRequest, BatchedExtensionResponse, EntityExtensionDataArray,
};
use librespot_protocol::extension_kind::ExtensionKind;
use librespot_protocol::metadata::{image::Size, Album, Artist, Disc, Image, ImageGroup, Track};
use librespot_protocol::playlist4_external::{
    Item, ListAttributes, ListItems, MetaItem, PictureSize, SelectedListContent,
};
use protobuf::well_known_types::any::Any;
use protobuf::Message;

use super::cache::{lookup, map_for, PerAccount};
use super::contents::{self, Contents, Page};
use super::covers::pick;
use super::error::LibraryError;
use super::gate::{retry_after, Gate, DEFAULT_WAIT};
use super::metadata::{self, TrackRow};
use super::rootlist::{
    self, encode, parse_page, parse_playlist, playlist_endpoint, playlist_uri, Entry, Header,
};
use super::search::{self, clean, parse, SearchKind, MAX_QUERY};
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

/// A `searchDesktop` answer in the shape the web player reads, trimmed: a
/// null hit, a hit without a URI, a `NotFound` hit, a playlist owned by
/// Spotify, an artist without a name, plain-http and sized image sources.
const SEARCH: &str = r#"{
  "data": { "searchV2": {
    "tracksV2": { "totalCount": 800, "items": [
      { "item": { "data": { "__typename": "Track", "uri": "spotify:track:tr1", "name": "So What",
          "albumOfTrack": { "uri": "spotify:album:al1", "name": "Kind of Blue",
            "coverArt": { "sources": [
              { "url": "https://i.scdn.co/image/a64", "width": 64 },
              { "url": "https://i.scdn.co/image/a640", "width": 640 },
              { "url": "https://i.scdn.co/image/a300", "width": 300 } ] } },
          "artists": { "items": [
            { "uri": "spotify:artist:ar1", "profile": { "name": "Miles Davis" } },
            { "uri": "spotify:artist:ar2", "profile": { "name": "John Coltrane" } } ] } } } },
      null,
      { "item": { "data": { "__typename": "Track", "uri": "spotify:track:tr2", "name": "Loose",
          "artists": { "items": [] } } } },
      { "item": { "data": { "__typename": "Track", "name": "No uri" } } },
      { "item": { "data": { "__typename": "NotFound" } } }
    ] },
    "albumsV2": { "totalCount": 1, "items": [
      { "data": { "__typename": "Album", "uri": "spotify:album:al1", "name": "Kind of Blue",
          "coverArt": { "sources": [ { "url": "http://insecure.example/a", "width": 640 } ] },
          "artists": { "items": [ { "profile": { "name": "Miles Davis" } } ] } } }
    ] },
    "playlists": { "totalCount": 2, "items": [
      { "data": { "__typename": "Playlist", "uri": "spotify:playlist:pl1", "name": "Jazz Classics",
          "ownerV2": { "data": { "__typename": "User", "name": "Spotify" } },
          "images": { "items": [ { "sources": [ { "url": "https://i.scdn.co/image/p1", "width": null } ] } ] } } },
      { "data": { "__typename": "Playlist", "uri": "spotify:playlist:pl2", "name": "" } }
    ] },
    "artists": { "items": [
      { "data": { "__typename": "Artist", "uri": "spotify:artist:ar1",
          "profile": { "name": "Miles Davis" },
          "visuals": { "avatarImage": { "sources": [ { "url": "https://i.scdn.co/image/m320", "width": 320 } ] } } } },
      { "data": { "__typename": "Artist", "uri": "spotify:artist:ar3", "profile": {} } }
    ] }
  } },
  "extensions": {}
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
    assert_eq!(r.artists.len(), 1);
    assert_eq!(r.artists[0].name, "Miles Davis");
    assert_eq!(r.artists[0].subtitle, "Artist");
    assert_eq!(
        r.artists[0].image_url.as_deref(),
        Some("https://i.scdn.co/image/m320")
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
    assert_eq!(
        parse(br#"{"data":{"searchV2":{}}}"#).unwrap(),
        Default::default()
    );
    assert!(matches!(parse(b"<html>"), Err(LibraryError::Unreadable(_))));
}

#[test]
fn a_stale_hash_reads_as_search_outdated_and_other_errors_surface() {
    assert_eq!(
        parse(br#"{"errors":[{"message":"PersistedQueryNotFound"}]}"#),
        Err(LibraryError::SearchOutdated)
    );
    assert_eq!(
        parse(br#"{"errors":[{"message":"Variable missing"}],"data":null}"#),
        Err(LibraryError::Unreadable("Variable missing".into()))
    );
    assert_eq!(
        search::unavailable(LibraryError::Status(412)),
        LibraryError::SearchOutdated
    );
}

#[test]
fn a_page_reads_its_own_group_and_the_next_offset() {
    let page = search::parse_page(SEARCH.as_bytes(), SearchKind::Albums).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].uri, "spotify:album:al1");
    assert_eq!(page.next_offset, None);
    let body = br#"{"data":{"searchV2":{"tracksV2":{"items":[
        {"item":{"data":{"__typename":"Track","uri":"spotify:track:t9","name":"Nine"}}}],
        "pagingInfo":{"nextOffset":30,"limit":20}}}}}"#;
    let page = search::parse_page(body, SearchKind::Tracks).unwrap();
    assert_eq!(page.items[0].name, "Nine");
    assert_eq!(page.next_offset, Some(30));
    assert_eq!(
        search::parse_page(
            br#"{"errors":[{"message":"PersistedQueryNotFound"}]}"#,
            SearchKind::Tracks
        ),
        Err(LibraryError::SearchOutdated)
    );
    let body: serde_json::Value =
        serde_json::from_slice(&search::page_body("lo-fi", SearchKind::Playlists, 40)).unwrap();
    assert_eq!(body["operationName"], "searchPlaylists");
    assert_eq!(body["variables"]["offset"], 40);
    assert_eq!(body["variables"]["limit"], 20);
}

#[test]
fn a_search_query_is_trimmed_capped_and_sent_as_search_desktop() {
    assert_eq!(clean("   "), None);
    assert_eq!(clean("  lo-fi ").as_deref(), Some("lo-fi"));
    assert_eq!(clean(&"é".repeat(300)).unwrap().chars().count(), MAX_QUERY);
    let body: serde_json::Value =
        serde_json::from_slice(&search::body("kind of \"blue\"")).unwrap();
    assert_eq!(body["operationName"], "searchDesktop");
    assert_eq!(body["variables"]["searchTerm"], "kind of \"blue\"");
    assert_eq!(body["variables"]["limit"], 10);
    assert_eq!(body["extensions"]["persistedQuery"]["version"], 1);
    assert_eq!(
        body["extensions"]["persistedQuery"]["sha256Hash"]
            .as_str()
            .unwrap()
            .len(),
        64
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

const A: &str = "spotify:track:4uLU6hMCjMI75M1A2tKUQC";
const B: &str = "spotify:track:7ouMYWpwJ422jRcDASZB7P";
const C: &str = "spotify:track:0eGsygTp906u18L0Oimnem";
const D: &str = "spotify:track:3n3Ppam7vgaVa1iaRUc9Lp";
const E: &str = "spotify:track:6rqhFgbbKwnb9MLmUQDhG6";

fn image(id: &[u8], size: Size) -> Image {
    Image {
        file_id: Some(id.to_vec()),
        size: Some(size.into()),
        ..Image::default()
    }
}

fn track(name: &str, album_gid: &[u8], covers: Vec<Image>) -> Track {
    Track {
        name: Some(name.into()),
        duration: Some(201_000),
        artist: vec![
            Artist {
                name: Some("Miles Davis".into()),
                ..Artist::default()
            },
            Artist {
                name: Some("John Coltrane".into()),
                ..Artist::default()
            },
        ],
        album: Some(Album {
            gid: Some(album_gid.to_vec()),
            name: Some("Kind of Blue".into()),
            cover_group: Some(ImageGroup {
                image: covers,
                ..ImageGroup::default()
            })
            .into(),
            ..Album::default()
        })
        .into(),
        ..Track::default()
    }
}

/// A `get_extended_metadata` answer: one `TRACK_V4` array whose entities
/// each wrap a `metadata::Track` in an `Any`, one entity with no data.
fn metadata_bytes(tracks: Vec<(&str, Track)>) -> Vec<u8> {
    let mut data: Vec<EntityExtensionData> = tracks
        .into_iter()
        .map(|(uri, t)| EntityExtensionData {
            entity_uri: uri.into(),
            extension_data: Some(Any {
                type_url: "type.googleapis.com/spotify.metadata.Track".into(),
                value: t.write_to_bytes().unwrap(),
                ..Any::default()
            })
            .into(),
            ..EntityExtensionData::default()
        })
        .collect();
    data.push(EntityExtensionData {
        entity_uri: E.into(),
        ..EntityExtensionData::default()
    });
    BatchedExtensionResponse {
        extended_metadata: vec![EntityExtensionDataArray {
            extension_kind: ExtensionKind::TRACK_V4.into(),
            extension_data: data,
            ..EntityExtensionDataArray::default()
        }],
        ..BatchedExtensionResponse::default()
    }
    .write_to_bytes()
    .unwrap()
}

#[test]
fn an_albums_tracks_list_disc_by_disc_and_skip_tracks_without_a_gid() {
    let id = |b: u8| [b; 16].to_vec();
    let disc = |gids: Vec<Vec<u8>>| Disc {
        track: gids
            .into_iter()
            .map(|gid| Track {
                gid: (!gid.is_empty()).then_some(gid),
                ..Track::default()
            })
            .collect(),
        ..Disc::default()
    };
    let album = Album {
        disc: vec![disc(vec![id(1), Vec::new()]), disc(vec![id(2)])],
        ..Album::default()
    };
    let body = BatchedExtensionResponse {
        extended_metadata: vec![EntityExtensionDataArray {
            extension_kind: ExtensionKind::ALBUM_V4.into(),
            extension_data: vec![EntityExtensionData {
                entity_uri: "spotify:album:al1".into(),
                extension_data: Some(Any {
                    type_url: "type.googleapis.com/spotify.metadata.Album".into(),
                    value: album.write_to_bytes().unwrap(),
                    ..Any::default()
                })
                .into(),
                ..EntityExtensionData::default()
            }],
            ..EntityExtensionDataArray::default()
        }],
        ..BatchedExtensionResponse::default()
    }
    .write_to_bytes()
    .unwrap();
    let uris = metadata::album_tracks(&body).unwrap();
    let base62 = |b: u8| SpotifyId::from_raw(&[b; 16]).unwrap().to_base62();
    assert_eq!(
        uris,
        [
            format!("spotify:track:{}", base62(1)),
            format!("spotify:track:{}", base62(2)),
        ]
    );
    assert!(metadata::album_tracks(
        &BatchedExtensionResponse::default()
            .write_to_bytes()
            .unwrap()
    )
    .unwrap()
    .is_empty());
}

#[test]
fn the_metadata_request_asks_track_v4_for_each_track() {
    assert_eq!(
        metadata::ENDPOINT,
        "/extended-metadata/v0/extended-metadata"
    );
    let body = metadata::request(&[A.to_string(), B.to_string()], ExtensionKind::TRACK_V4);
    let request = BatchedEntityRequest::parse_from_bytes(&body).unwrap();
    let asked: Vec<(&str, Vec<ExtensionKind>)> = request
        .entity_request
        .iter()
        .map(|e| {
            let kinds = e.query.iter().map(|q| q.extension_kind.unwrap()).collect();
            (e.entity_uri.as_str(), kinds)
        })
        .collect();
    assert_eq!(
        asked,
        [
            (A, vec![ExtensionKind::TRACK_V4]),
            (B, vec![ExtensionKind::TRACK_V4])
        ]
    );
    assert!(request.header.is_none());
}

#[test]
fn a_metadata_answer_decodes_into_track_rows() {
    let body = metadata_bytes(vec![
        (
            A,
            track(
                "So What",
                &[0x0a, 0x0b],
                vec![
                    image(&[0x64], Size::SMALL),
                    image(&[0x30, 0x0f], Size::DEFAULT),
                    image(&[0x64, 0x40], Size::LARGE),
                ],
            ),
        ),
        // No DEFAULT size: the largest.
        (
            B,
            track(
                "Blue in Green",
                &[0x0c],
                vec![image(&[0x01], Size::SMALL), image(&[0x02], Size::XLARGE)],
            ),
        ),
        // No name: not a row.
        (C, track("", &[0x0d], vec![])),
    ]);
    let rows = metadata::parse(&body).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[A],
        TrackRow {
            uri: A.into(),
            name: "So What".into(),
            artists: vec!["Miles Davis".into(), "John Coltrane".into()],
            album: "Kind of Blue".into(),
            image_url: Some("https://i.scdn.co/image/300f".into()),
            duration_ms: 201_000,
            album_key: "0a0b".into(),
        }
    );
    assert_eq!(
        rows[B].image_url.as_deref(),
        Some("https://i.scdn.co/image/02")
    );
    // The album key never reaches the frontend.
    let json = serde_json::to_value(&rows[A]).unwrap();
    assert!(json.get("album_key").is_none());
    assert!(matches!(
        metadata::parse(b"\xff\xff\xff"),
        Err(LibraryError::Unreadable(_))
    ));
}

fn row(uri: &str, album: &str, image: Option<&str>) -> (String, TrackRow) {
    let row = TrackRow {
        uri: uri.into(),
        name: "t".into(),
        artists: vec![],
        album: String::new(),
        image_url: image.map(Into::into),
        duration_ms: 0,
        album_key: album.into(),
    };
    (uri.to_string(), row)
}

fn uris(list: &[&str]) -> Vec<String> {
    list.iter().map(|u| u.to_string()).collect()
}

#[test]
fn a_cover_is_four_distinct_albums_else_the_first_alone() {
    let rows: HashMap<String, TrackRow> = [
        row(A, "al1", Some("u1")),
        row(B, "al1", Some("u1-again")),
        row(C, "al2", Some("u2")),
        row(D, "al3", Some("u3")),
        row(E, "al4", Some("u4")),
        row("spotify:track:nocover", "al5", None),
    ]
    .into_iter()
    .collect();
    // Empty, or no rows: none.
    assert!(pick(&[], &rows).is_empty());
    assert!(pick(&uris(&["spotify:track:unknown"]), &rows).is_empty());
    // One album, or three: the first track's alone.
    assert_eq!(pick(&uris(&[A, B]), &rows), ["u1"]);
    assert_eq!(pick(&uris(&[A, C, D]), &rows), ["u1"]);
    // A coverless first track is passed over.
    assert_eq!(pick(&uris(&["spotify:track:nocover", C]), &rows), ["u2"]);
    // Four distinct albums, a repeated one skipped, the rest unread.
    assert_eq!(
        pick(&uris(&[A, B, C, D, E, A]), &rows),
        ["u1", "u2", "u3", "u4"]
    );
}

fn list_bytes(uris: &[&str], pos: i32, truncated: bool) -> Vec<u8> {
    SelectedListContent {
        contents: Some(ListItems {
            pos: Some(pos),
            truncated: Some(truncated),
            items: uris.iter().map(|u| item(u)).collect(),
            ..ListItems::default()
        })
        .into(),
        ..SelectedListContent::default()
    }
    .write_to_bytes()
    .unwrap()
}

#[test]
fn a_playlists_contents_keep_catalogue_tracks_in_order() {
    let body = list_bytes(
        &[
            A,
            "spotify:local:Artist:Album:Song:180",
            "spotify:episode:5Xt5DXGzch68nYYamXrNxZ",
            B,
            "spotify:track:short",
        ],
        0,
        true,
    );
    let page = contents::parse_page(&body).unwrap();
    assert_eq!(
        page,
        Page {
            tracks: uris(&[A, B]),
            pos: 0,
            items: 5,
            truncated: true,
        }
    );
    assert_eq!(
        contents::page_endpoint("spotify:playlist:3c4d", 500).as_deref(),
        Some("/playlist/v2/playlist/3c4d?from=500&length=500")
    );
    assert_eq!(contents::page_endpoint("spotify:album:x", 0), None);
}

#[test]
fn contents_page_on_while_truncated_and_stop_when_paging_is_ignored() {
    let page = |tracks: &[&str], pos, items, truncated| Page {
        tracks: uris(tracks),
        pos,
        items,
        truncated,
    };
    // Whole in one answer.
    assert_eq!(Contents::first(page(&[A], 0, 1, false)).next, None);
    let mut list = Contents::first(page(&[A, B], 0, 3, true));
    assert_eq!(list.next, Some(3));
    list.extend(3, page(&[C], 3, 2, true));
    assert_eq!(
        (list.tracks.clone(), list.next),
        (uris(&[A, B, C]), Some(5))
    );
    list.extend(5, page(&[D], 5, 1, false));
    assert_eq!((list.tracks.len(), list.next), (4, None));
    // A page that starts at the top again: `from` was ignored.
    let mut list = Contents::first(page(&[A], 0, 1, true));
    list.extend(1, page(&[A], 0, 1, true));
    assert_eq!((list.tracks, list.next), (uris(&[A]), None));
    // An empty page ends it too.
    let mut list = Contents::first(page(&[A], 0, 1, true));
    list.extend(1, page(&[], 1, 0, true));
    assert_eq!(list.next, None);
}

#[test]
fn caches_belong_to_one_account() {
    let mut slot: PerAccount<u32> = None;
    map_for(&mut slot, "a").insert("x".into(), 1);
    assert_eq!(lookup(&slot, "a", "x"), Some(1));
    assert_eq!(lookup(&slot, "b", "x"), None);
    // Another account starts afresh.
    map_for(&mut slot, "b").insert("y".into(), 2);
    assert_eq!(lookup(&slot, "a", "x"), None);
    assert_eq!(lookup(&slot, "b", "y"), Some(2));
}

#[test]
fn only_refusals_and_unreadable_answers_are_lasting() {
    for e in [
        LibraryError::Forbidden,
        LibraryError::Status(404),
        LibraryError::Unreadable("x".into()),
    ] {
        assert!(e.is_lasting(), "{e:?}");
    }
    for e in [
        LibraryError::RateLimited(3),
        LibraryError::Transport("x".into()),
        LibraryError::Token("x".into()),
        LibraryError::Unauthorized,
    ] {
        assert!(!e.is_lasting(), "{e:?}");
    }
}

#[test]
fn a_playlist_shows_its_header_picture_as_a_one_cover_image_list() {
    let header = Header {
        name: "n".into(),
        owner: None,
        track_count: None,
        image_url: Some("https://i.scdn.co/image/ab".into()),
    };
    assert_eq!(
        header.clone().playlist("spotify:playlist:a".into()).images,
        ["https://i.scdn.co/image/ab"]
    );
    let bare = Header {
        image_url: None,
        ..header
    };
    assert!(bare.playlist("spotify:playlist:a".into()).images.is_empty());
}

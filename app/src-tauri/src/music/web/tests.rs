use std::time::{Duration, Instant, SystemTime};

use super::devices;
use super::error::WebError;
use super::gate::{retry_after, Gate, DEFAULT_WAIT};
use super::http::{status_error, API};
use super::playlists::{parse_page, Playlist};
use super::search::{clean, parse, url, MAX_QUERY};
use super::token::cached;

/// A `/v1/me/playlists` page in the shape Spotify sends, trimmed: a null
/// entry, a stub without a URI, a mosaic cover without widths, the newer
/// `items` count, an owner without a display name.
const PLAYLIST_PAGE: &str = r#"{
  "href": "https://api.spotify.com/v1/me/playlists?offset=0&limit=50",
  "limit": 50,
  "next": "https://api.spotify.com/v1/me/playlists?offset=50&limit=50",
  "offset": 0,
  "previous": null,
  "total": 3,
  "items": [
    {
      "collaborative": false,
      "description": "",
      "id": "37i9dQZF1DXcBWIGoYBM5M",
      "images": [
        { "height": 640, "url": "https://i.scdn.co/image/ab67706f00000002aa", "width": 640 },
        { "height": 300, "url": "https://i.scdn.co/image/ab67706f00000003bb", "width": 300 },
        { "height": 60, "url": "https://i.scdn.co/image/ab67706f00000004cc", "width": 60 }
      ],
      "name": "Focus",
      "owner": { "display_name": "Spotify", "id": "spotify", "type": "user" },
      "public": true,
      "tracks": { "href": "https://api.spotify.com/v1/playlists/x/tracks", "total": 91 },
      "type": "playlist",
      "uri": "spotify:playlist:37i9dQZF1DXcBWIGoYBM5M"
    },
    null,
    { "name": "Broken", "uri": null, "images": null },
    {
      "id": "1a2b",
      "images": [
        { "height": null, "url": "https://mosaic.scdn.co/640/ab67616d0000b273ff", "width": null }
      ],
      "name": "Study mix",
      "owner": { "display_name": null, "id": "tanat" },
      "items": { "href": "https://api.spotify.com/v1/playlists/1a2b/items", "total": 12 },
      "uri": "spotify:playlist:1a2b"
    },
    {
      "id": "3c4d",
      "images": [],
      "name": "Empty",
      "owner": { "display_name": "Tanat", "id": "tanat" },
      "tracks": { "total": 0 },
      "uri": "spotify:playlist:3c4d"
    }
  ]
}"#;

#[test]
fn a_playlist_page_parses_and_skips_nulls_and_stubs() {
    let page = parse_page(PLAYLIST_PAGE).unwrap();
    assert_eq!(
        page.next.as_deref(),
        Some("https://api.spotify.com/v1/me/playlists?offset=50&limit=50")
    );
    assert_eq!(
        page.items,
        [
            Playlist {
                uri: "spotify:playlist:37i9dQZF1DXcBWIGoYBM5M".into(),
                name: "Focus".into(),
                owner: Some("Spotify".into()),
                track_count: Some(91),
                // The smallest at least 120 wide.
                image_url: Some("https://i.scdn.co/image/ab67706f00000003bb".into()),
            },
            Playlist {
                uri: "spotify:playlist:1a2b".into(),
                name: "Study mix".into(),
                owner: Some("tanat".into()),
                track_count: Some(12),
                image_url: Some("https://mosaic.scdn.co/640/ab67616d0000b273ff".into()),
            },
            Playlist {
                uri: "spotify:playlist:3c4d".into(),
                name: "Empty".into(),
                owner: Some("Tanat".into()),
                track_count: Some(0),
                image_url: None,
            },
        ]
    );
}

#[test]
fn the_last_playlist_page_has_no_next_and_junk_is_unreadable() {
    let page = parse_page(r#"{"items":[],"next":null}"#).unwrap();
    assert!(page.items.is_empty());
    assert!(page.next.is_none());
    assert!(matches!(parse_page("<html>"), Err(WebError::Unreadable(_))));
}

/// `/v1/search?type=playlist,album,track`, trimmed, with the null entries
/// Spotify puts in its playlist results.
const SEARCH: &str = r#"{
  "playlists": { "items": [
    null,
    { "name": "Lo-fi beats", "uri": "spotify:playlist:pl1",
      "owner": { "display_name": "Spotify", "id": "spotify" },
      "images": [{ "url": "https://i.scdn.co/image/p1", "width": null, "height": null }],
      "tracks": { "total": 450 } }
  ] },
  "albums": { "items": [
    { "name": "Kind of Blue", "uri": "spotify:album:al1",
      "artists": [{ "name": "Miles Davis" }],
      "images": [
        { "url": "https://i.scdn.co/image/a640", "width": 640, "height": 640 },
        { "url": "https://i.scdn.co/image/a300", "width": 300, "height": 300 },
        { "url": "http://insecure.example/a64", "width": 64, "height": 64 }
      ] }
  ] },
  "tracks": { "items": [
    { "name": "So What", "uri": "spotify:track:tr1",
      "artists": [{ "name": "Miles Davis" }, { "name": "John Coltrane" }],
      "album": { "name": "Kind of Blue", "uri": "spotify:album:al1", "artists": [],
        "images": [{ "url": "https://i.scdn.co/image/a300", "width": 300, "height": 300 }] } },
    { "name": "Loose", "uri": "spotify:track:tr2", "artists": [] }
  ] }
}"#;

#[test]
fn search_results_group_with_subtitles_and_a_tracks_album() {
    let r = parse(SEARCH).unwrap();
    assert_eq!(r.playlists.len(), 1);
    assert_eq!(r.playlists[0].subtitle, "Spotify");
    assert_eq!(
        r.playlists[0].image_url.as_deref(),
        Some("https://i.scdn.co/image/p1")
    );
    assert_eq!(r.albums[0].subtitle, "Miles Davis");
    assert_eq!(
        r.albums[0].image_url.as_deref(),
        Some("https://i.scdn.co/image/a300")
    );
    assert_eq!(
        r.tracks[0].subtitle,
        "Miles Davis, John Coltrane · Kind of Blue"
    );
    assert_eq!(
        r.tracks[0].context_uri.as_deref(),
        Some("spotify:album:al1")
    );
    assert_eq!(r.tracks[1].subtitle, "");
    assert_eq!(r.tracks[1].context_uri, None);
    assert_eq!(r.tracks[1].image_url, None);
    // A group Spotify leaves out is empty.
    assert_eq!(parse("{}").unwrap(), Default::default());
}

#[test]
fn a_search_query_is_trimmed_capped_and_encoded() {
    assert_eq!(clean("   "), None);
    assert_eq!(clean("  lo-fi ").as_deref(), Some("lo-fi"));
    assert_eq!(clean(&"é".repeat(300)).unwrap().chars().count(), MAX_QUERY);
    let u = url("kind of blue & more");
    assert!(u.starts_with(&format!("{API}search?")), "{u}");
    assert!(u.contains("q=kind+of+blue+%26+more"), "{u}");
    assert!(u.contains("type=playlist%2Calbum%2Ctrack"), "{u}");
    assert!(u.contains("limit=10"), "{u}");
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
    assert_eq!(status_error(401, None), WebError::Unauthorized);
    assert_eq!(status_error(403, None), WebError::Forbidden);
    assert_eq!(status_error(429, Some("40")), WebError::RateLimited(40));
    assert_eq!(status_error(502, None), WebError::Status(502));
    assert_eq!(
        WebError::RateLimited(40).to_string(),
        "Spotify is rate-limiting Oculus; try again in 40 s"
    );
    assert_eq!(
        WebError::Forbidden.to_string(),
        "Spotify refused the request (403)"
    );
    assert_eq!(
        WebError::NotConnected.to_string(),
        "Not connected to Spotify"
    );
}

#[test]
fn a_token_is_reused_only_for_its_session_and_before_its_last_minute() {
    let now = SystemTime::now();
    let token = cached(3, "t", now + Duration::from_secs(3600));
    assert!(token.usable(3, now));
    assert!(!token.usable(4, now));
    assert!(!token.usable(3, now + Duration::from_secs(3541)));
}

/// `/v1/me/player/devices`, trimmed: a device without an id, this Oculus
/// under a Web API id of its own, a null volume.
const DEVICES: &str = r#"{ "devices": [
  { "id": "a1", "is_active": true, "is_private_session": false, "is_restricted": false,
    "name": "Pixel 9", "supports_volume": true, "type": "Smartphone", "volume_percent": 60 },
  { "id": null, "is_active": false, "name": "Restricted", "type": "Speaker", "volume_percent": 20 },
  { "id": "web-oculus", "is_active": false, "name": "Oculus", "type": "Computer", "volume_percent": 100 },
  { "id": "b2", "is_active": false, "name": "Living Room", "type": "CastAudio", "volume_percent": null }
] }"#;

#[test]
fn the_device_list_parses_and_marks_this_device_with_its_own_id() {
    let devices = devices::parse(DEVICES, "own").unwrap();
    let names: Vec<_> = devices.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["Oculus", "Pixel 9", "Living Room"]);
    assert!(devices[0].is_this);
    assert_eq!(devices[0].id, "own");
    assert!(devices[1].active);
    assert_eq!((devices[1].kind, devices[1].volume), ("phone", Some(60)));
    assert_eq!((devices[2].kind, devices[2].volume), ("speaker", None));
    // An id match wins over the name.
    let by_id = devices::parse(
        r#"{"devices":[{"id":"own","name":"Desk","type":"Computer"},{"id":"x","name":"Oculus","type":"Computer"}]}"#,
        "own",
    )
    .unwrap();
    assert!(by_id.iter().find(|d| d.name == "Desk").unwrap().is_this);
    assert!(!by_id.iter().find(|d| d.name == "Oculus").unwrap().is_this);
}

#[test]
fn a_transfer_asks_spotify_to_play_on_the_device() {
    let body: serde_json::Value = serde_json::from_str(&devices::transfer_body("a1")).unwrap();
    assert_eq!(
        body,
        serde_json::json!({ "device_ids": ["a1"], "play": true })
    );
}

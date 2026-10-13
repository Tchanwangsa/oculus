use serde_json::{json, Value};

use super::list::{self, Item};
use super::merge::{self, Dated};
use crate::music::library::error::LibraryError;
use crate::music::recent_tracks::{Played, CAP};

const SO_WHAT: &str = "spotify:track:4vLYewWIvqHfKtJDk8c8tq";
const BLUE: &str = "spotify:track:0ZqX6CUnsgiGkaFMTgTYgt";
const FREDDIE: &str = "spotify:track:2Ffpj4hbKSOzd8Yz0IpFxq";
const KIND_OF_BLUE: &str = "spotify:album:1weenld61qoidwYuZ1GESA";

const DAY_MS: u64 = 86_400_000;
/// 2026-10-12 and the days around it, as days since the epoch.
const OCT_12: i64 = 20_738;

/// A list entry the way the web player's reader sees one.
fn entry(uri: &str, kind: &str, added: Value) -> Value {
    json!({
        "entity": {
            "_uri": uri,
            "data": {
                "__typename": "Entity",
                "identityTrait": {
                    "__typename": "IdentityTrait",
                    "name": "Name",
                    "contributors": { "items": [] },
                    "contentHierarchyParent": {
                        "__typename": "Entity",
                        "uri": KIND_OF_BLUE,
                        "identityTrait": { "name": "Kind of Blue" }
                    }
                },
                "visualIdentityTrait": {},
                "entityTypeTrait": { "__typename": "EntityTypeTrait", "type": kind },
                "consumptionExperienceTrait": {
                    "duration": { "seconds": 562, "nanoSeconds": 0 }
                }
            }
        },
        "addedAt": added,
        "formatListAttributes": []
    })
}

fn answer(items: Vec<Value>, next_offset: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "data": {
            "lists": [{
                "__typename": "List",
                "uri": "spotify:list:recents:page",
                "name": "Recents",
                "items": {
                    "totalCount": 120,
                    "pagingInfo": { "offset": 0, "limit": 50, "nextOffset": next_offset },
                    "items": items
                }
            }]
        }
    }))
    .unwrap()
}

fn item(uri: &str, kind: Option<&str>, day: Option<i64>) -> Item {
    Item {
        uri: uri.into(),
        kind: kind.map(Into::into),
        day,
    }
}

fn dated(uri: &str, day: i64) -> Dated {
    Dated {
        uri: uri.into(),
        day,
    }
}

fn played(uri: &str, at: u64) -> Played {
    Played {
        uri: uri.into(),
        played_at_ms: at,
    }
}

/// A track URI numbered `i`, 22 characters after the prefix.
fn numbered(i: u64) -> String {
    format!("spotify:track:{i:0>22}")
}

/// `ms` into day `day`.
fn on(day: i64, ms: u64) -> u64 {
    day as u64 * DAY_MS + ms
}

#[test]
fn the_request_asks_for_the_recents_page() {
    let body: Value = serde_json::from_slice(&list::body(50)).unwrap();
    assert_eq!(body["operationName"], "recents");
    assert_eq!(
        body["variables"],
        json!({ "uris": ["spotify:list:recents:page"], "offset": 50, "limit": 50 })
    );
    assert_eq!(
        body["extensions"]["persistedQuery"]["sha256Hash"],
        "698be5892a3cc95331deebeff463d05dfdd5febf5254bea30b895b5a93dfb584"
    );
}

#[test]
fn a_recents_page_reads_every_entry_with_its_type_and_day() {
    let body = answer(
        vec![
            entry(
                SO_WHAT,
                "ENTITY_TYPE_TRACK",
                json!({ "year": 2026, "month": 10, "day": 12 }),
            ),
            entry(
                KIND_OF_BLUE,
                "ENTITY_TYPE_ALBUM",
                json!({ "year": 2026, "month": 10, "day": 12 }),
            ),
            entry(
                "spotify:episode:512ojhOuo1ktJprKbVcKyQ",
                "ENTITY_TYPE_PODCAST_EPISODE",
                json!({ "year": 2026, "month": 10, "day": 11 }),
            ),
            // A missing day is the first of the month; a missing month, of
            // the year.
            entry(
                BLUE,
                "ENTITY_TYPE_TRACK",
                json!({ "year": 2026, "month": 10 }),
            ),
            entry(FREDDIE, "ENTITY_TYPE_TRACK", json!({ "year": 2026 })),
            // No date at all, and no type.
            json!({ "entity": { "_uri": numbered(1) } }),
            Value::Null,
            json!({ "entity": null }),
            json!({ "entity": { "_uri": 42 } }),
        ],
        json!(50),
    );
    let page = list::parse_page(&body).unwrap();
    assert_eq!(page.received, 9);
    assert_eq!(
        page.items,
        vec![
            item(SO_WHAT, Some("ENTITY_TYPE_TRACK"), Some(OCT_12)),
            item(KIND_OF_BLUE, Some("ENTITY_TYPE_ALBUM"), Some(OCT_12)),
            item(
                "spotify:episode:512ojhOuo1ktJprKbVcKyQ",
                Some("ENTITY_TYPE_PODCAST_EPISODE"),
                Some(OCT_12 - 1)
            ),
            item(BLUE, Some("ENTITY_TYPE_TRACK"), Some(OCT_12 - 11)),
            item(FREDDIE, Some("ENTITY_TYPE_TRACK"), Some(20_454)),
            item(&numbered(1), None, None),
        ]
    );
    let tracks: Vec<&str> = page
        .items
        .iter()
        .filter(|i| i.is_track())
        .map(|i| i.uri.as_str())
        .collect();
    let untyped = numbered(1);
    assert_eq!(tracks, vec![SO_WHAT, BLUE, FREDDIE, untyped.as_str()]);
}

#[test]
fn a_track_uri_typed_as_something_else_is_not_a_track() {
    assert!(!item(SO_WHAT, Some("ENTITY_TYPE_ALBUM"), None).is_track());
    assert!(!item("spotify:track:short", Some("ENTITY_TYPE_TRACK"), None).is_track());
    assert!(item(SO_WHAT, Some("ENTITY_TYPE_TRACK"), None).is_track());
}

#[test]
fn pages_follow_next_offset() {
    let page = |next: Value| list::parse_page(&answer(vec![entry(SO_WHAT, "x", json!({}))], next));
    assert_eq!(page(json!(50)).unwrap().next(0), Some(50));
    assert_eq!(page(json!(null)).unwrap().next(0), None);
    // An offset that goes nowhere ends it.
    assert_eq!(page(json!(50)).unwrap().next(50), None);
    let absent = br#"{"data":{"lists":[{"__typename":"List","items":{"items":[{}]}}]}}"#;
    assert_eq!(list::parse_page(absent).unwrap().next(0), None);
    let empty = answer(vec![], json!(50));
    assert_eq!(list::parse_page(&empty).unwrap().next(0), None);
}

#[test]
fn an_answer_without_a_list_fails() {
    let wrong = br#"{"data":{"lists":[{"__typename":"NotFound"}]}}"#;
    assert!(matches!(
        list::parse_page(wrong),
        Err(LibraryError::Unreadable(m)) if m.contains("NotFound")
    ));
    let nameless = br#"{"data":{"lists":[{"items":{"items":[]}}]}}"#;
    assert!(list::parse_page(nameless).is_err());
    let stale = br#"{"errors":[{"message":"PersistedQueryNotFound"}],"data":null}"#;
    assert!(matches!(
        list::parse_page(stale),
        Err(LibraryError::Unreadable(m)) if m.contains("PersistedQueryNotFound")
    ));
    let other = br#"{"errors":[{"message":"boom"}]}"#;
    assert!(matches!(
        list::parse_page(other),
        Err(LibraryError::Unreadable(m)) if m == "boom"
    ));
    assert!(list::parse_page(br#"{"data":{"lists":[]}}"#).is_err());
    assert!(list::parse_page(b"not json").is_err());
}

#[test]
fn undated_tracks_take_the_day_before_them() {
    let tracks = vec![
        (SO_WHAT.to_string(), None),
        (BLUE.to_string(), Some(OCT_12)),
        (FREDDIE.to_string(), None),
        (numbered(1), Some(OCT_12 - 2)),
    ];
    assert_eq!(
        merge::dated(tracks),
        vec![
            dated(SO_WHAT, OCT_12),
            dated(BLUE, OCT_12),
            dated(FREDDIE, OCT_12),
            dated(&numbered(1), OCT_12 - 2),
        ]
    );
    assert_eq!(
        merge::dated(vec![(SO_WHAT.to_string(), None)]),
        vec![dated(SO_WHAT, i64::MIN)]
    );
}

#[test]
fn days_group_the_lists_with_oculuss_own_first_in_each() {
    let spotify = vec![
        dated(SO_WHAT, OCT_12),
        dated(BLUE, OCT_12),
        dated(FREDDIE, OCT_12 - 1),
    ];
    let local = vec![
        // Newer than anything Spotify lists: on top.
        played(&numbered(1), on(OCT_12 + 1, 5)),
        played(&numbered(2), on(OCT_12, 9)),
        played(&numbered(3), on(OCT_12, 7)),
        played(&numbered(4), on(OCT_12 - 1, 1)),
    ];
    let uris: Vec<String> = merge::merge(&spotify, &local)
        .into_iter()
        .map(|p| p.uri)
        .collect();
    assert_eq!(
        uris,
        vec![
            numbered(1),
            numbered(2),
            numbered(3),
            SO_WHAT.into(),
            BLUE.into(),
            numbered(4),
            FREDDIE.into(),
        ]
    );
}

#[test]
fn a_track_in_both_lists_shows_once_at_its_first_place() {
    let spotify = vec![
        dated(SO_WHAT, OCT_12),
        dated(BLUE, OCT_12),
        dated(FREDDIE, OCT_12 - 2),
    ];
    let local = vec![
        // Spotify has it today too: Spotify's place.
        played(BLUE, on(OCT_12, 9)),
        // Spotify has it on an older day: today's place, after Spotify's.
        played(FREDDIE, on(OCT_12, 8)),
        // Older here than in Spotify's list: Spotify's place.
        played(SO_WHAT, on(OCT_12 - 3, 1)),
    ];
    let merged = merge::merge(&spotify, &local);
    assert_eq!(
        merged,
        vec![
            played(SO_WHAT, on(OCT_12, 0)),
            played(BLUE, on(OCT_12, 0)),
            played(FREDDIE, on(OCT_12, 8)),
        ]
    );
}

#[test]
fn the_merge_is_capped() {
    let spotify: Vec<Dated> = (0..40).map(|i| dated(&numbered(i), OCT_12)).collect();
    let local: Vec<Played> = (100..130)
        .map(|i| played(&numbered(i), on(OCT_12, 1_000 - i)))
        .collect();
    let merged = merge::merge(&spotify, &local);
    assert_eq!(merged.len(), CAP);
    assert_eq!(merged[0].uri, numbered(100));
    assert_eq!(merged[29].uri, numbered(129));
    assert_eq!(merged[30].uri, numbered(0));
    assert_eq!(merged[CAP - 1].uri, numbered(19));
}

#[test]
fn without_spotifys_list_oculuss_own_shows_alone() {
    let local = vec![played(BLUE, on(OCT_12, 1)), played(SO_WHAT, on(OCT_12, 9))];
    assert_eq!(
        merge::merge(&[], &local),
        vec![played(SO_WHAT, on(OCT_12, 9)), played(BLUE, on(OCT_12, 1))]
    );
    assert!(merge::merge(&[], &[]).is_empty());
}

#[test]
fn an_undated_spotify_list_follows_oculuss_own() {
    let spotify = vec![dated(SO_WHAT, i64::MIN)];
    let merged = merge::merge(&spotify, &[played(BLUE, on(OCT_12, 1))]);
    assert_eq!(
        merged,
        vec![played(BLUE, on(OCT_12, 1)), played(SO_WHAT, 0)]
    );
}

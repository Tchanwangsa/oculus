use super::recorder::AGAIN_AFTER_MS;
use super::{encode, is_track, parse, record, History, Played, Recorder, CAP};
use crate::music::state::{Snapshot, Track};

const SO_WHAT: &str = "spotify:track:4vLYewWIvqHfKtJDk8c8tq";
const BLUE: &str = "spotify:track:0ZqX6CUnsgiGkaFMTgTYgt";

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

/// Oculus active and playing `track` as `account`.
fn playing(account: &str, track: Option<&str>) -> Snapshot {
    Snapshot {
        connected: true,
        active: true,
        playing: true,
        account: Some(account.into()),
        track: track.map(|uri| Track {
            uri: uri.into(),
            name: "t".into(),
            artists: vec![],
            album: String::new(),
            cover_url: None,
            duration_ms: 0,
        }),
        context_uri: Some("spotify:album:1weenld61qoidwYuZ1GESA".into()),
        ..Snapshot::default()
    }
}

#[test]
fn only_track_uris_are_tracks() {
    assert!(is_track(SO_WHAT));
    assert!(!is_track("spotify:track:short"));
    assert!(!is_track("spotify:episode:4vLYewWIvqHfKtJDk8c8tq"));
    assert!(!is_track("spotify:track:4vLYewWIvqHfKtJDk8c8t!"));
    assert!(!is_track(""));
}

#[test]
fn the_row_round_trips_for_its_own_account_only() {
    let row = History {
        account: "alice".into(),
        items: vec![played(SO_WHAT, 20), played(BLUE, 10)],
    };
    let text = encode(&row);
    assert_eq!(parse(Some(&text), "alice"), row.items);
    assert!(parse(Some(&text), "bob").is_empty());
    assert!(parse(None, "alice").is_empty());
    assert!(parse(Some("not json"), "alice").is_empty());
    assert!(parse(Some(r#"{"account":"","items":[]}"#), "").is_empty());
}

#[test]
fn a_row_read_back_is_cleaned() {
    let raw = format!(
        r#"{{"account":"alice","items":[
        {{"uri":"{BLUE}","played_at_ms":5}},
        {{"uri":"spotify:album:1weenld61qoidwYuZ1GESA","played_at_ms":50}},
        {{"uri":"{SO_WHAT}","played_at_ms":30}},
        {{"uri":"{SO_WHAT}","played_at_ms":40}}
    ],"extra":true}}"#
    );
    assert_eq!(
        parse(Some(&raw), "alice"),
        vec![played(SO_WHAT, 40), played(BLUE, 5)]
    );
}

#[test]
fn recording_puts_a_track_first_once() {
    let items = vec![played(BLUE, 20), played(SO_WHAT, 10)];
    let items = record(items, played(SO_WHAT, 30));
    assert_eq!(items, vec![played(SO_WHAT, 30), played(BLUE, 20)]);
}

#[test]
fn recording_caps_the_row() {
    let mut items = Vec::new();
    for i in 0..(CAP as u64 + 5) {
        items = record(items, played(&numbered(i), i));
    }
    assert_eq!(items.len(), CAP);
    assert_eq!(items[0].uri, numbered(CAP as u64 + 4));
    assert_eq!(items[CAP - 1].uri, numbered(5));
}

#[test]
fn a_track_is_noted_when_oculus_starts_playing_it() {
    let mut r = Recorder::default();
    assert_eq!(
        r.due(&playing("alice", Some(SO_WHAT)), 1_000),
        Some(("alice".into(), played(SO_WHAT, 1_000)))
    );
    // Another change while it plays (a seek, the volume) notes nothing.
    assert_eq!(r.due(&playing("alice", Some(SO_WHAT)), 2_000), None);
    // The next track is noted at once.
    assert_eq!(
        r.due(&playing("alice", Some(BLUE)), 3_000),
        Some(("alice".into(), played(BLUE, 3_000)))
    );
    // And going back to the first, a minute after it was noted, again.
    assert_eq!(
        r.due(&playing("alice", Some(SO_WHAT)), 61_000),
        Some(("alice".into(), played(SO_WHAT, 61_000)))
    );
}

#[test]
fn a_quick_pause_and_resume_notes_nothing_new() {
    let mut r = Recorder::default();
    assert!(r.due(&playing("alice", Some(SO_WHAT)), 1_000).is_some());
    let paused = Snapshot {
        playing: false,
        ..playing("alice", Some(SO_WHAT))
    };
    assert_eq!(r.due(&paused, 2_000), None);
    assert_eq!(r.due(&playing("alice", Some(SO_WHAT)), 30_000), None);
    assert_eq!(r.due(&paused, 40_000), None);
    let later = 1_000 + AGAIN_AFTER_MS;
    assert_eq!(
        r.due(&playing("alice", Some(SO_WHAT)), later),
        Some(("alice".into(), played(SO_WHAT, later)))
    );
}

#[test]
fn nothing_is_noted_unless_oculus_plays_a_track() {
    let mut r = Recorder::default();
    let elsewhere = Snapshot {
        active: false,
        ..playing("alice", Some(SO_WHAT))
    };
    assert_eq!(r.due(&elsewhere, 1), None);
    assert_eq!(r.due(&playing("alice", None), 2), None);
    assert_eq!(
        r.due(
            &playing("alice", Some("spotify:episode:4vLYewWIvqHfKtJDk8c8tq")),
            3
        ),
        None
    );
    let signed_out = Snapshot {
        account: None,
        ..playing("alice", Some(SO_WHAT))
    };
    assert_eq!(r.due(&signed_out, 4), None);
}

#[test]
fn another_account_notes_the_same_track_again() {
    let mut r = Recorder::default();
    assert!(r.due(&playing("alice", Some(SO_WHAT)), 1_000).is_some());
    assert_eq!(
        r.due(&playing("bob", Some(SO_WHAT)), 2_000),
        Some(("bob".into(), played(SO_WHAT, 2_000)))
    );
}

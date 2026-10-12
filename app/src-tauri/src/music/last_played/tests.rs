use std::time::{Duration, Instant};

use super::memory::{advanced, LEFT_RECENTLY, WRITE_EVERY};
use super::{encode, parse, LastPlayed, Memory};
use crate::music::state::{Snapshot, Track};

const SONG: &str = "spotify:track:aaa";
const OTHER: &str = "spotify:track:bbb";
const LIST: &str = "spotify:playlist:x";

fn track(uri: &str) -> Track {
    Track {
        uri: uri.into(),
        name: "Song".into(),
        artists: vec!["Artist".into()],
        album: "Album".into(),
        cover_url: Some("https://i.scdn.co/image/ab".into()),
        duration_ms: 200_000,
    }
}

fn row(account: &str) -> LastPlayed {
    LastPlayed {
        account: account.into(),
        context_uri: Some(LIST.into()),
        track: track(SONG),
        position_ms: 61_000,
        saved_at_ms: 1_700_000_000_000,
        shuffle: true,
        repeat_context: false,
        repeat_track: true,
    }
}

/// Oculus active, playing `uri` from the playlist, at `position_ms` stamped
/// `stamp`.
fn playing(uri: &str, position_ms: u32, stamp: u64) -> Snapshot {
    Snapshot {
        connected: true,
        active: true,
        account: Some("alice".into()),
        track: Some(track(uri)),
        context_uri: Some(LIST.into()),
        position_ms,
        position_at_ms: stamp,
        playing: true,
        ..Snapshot::default()
    }
}

#[test]
fn the_row_round_trips_for_its_own_account_only() {
    let text = encode(&row("alice"));
    assert_eq!(parse(Some(&text), "alice"), Some(row("alice")));
    assert_eq!(parse(Some(&text), "bob"), None);
    assert_eq!(parse(None, "alice"), None);
}

#[test]
fn an_unreadable_or_empty_row_is_nothing() {
    for raw in [
        "",
        "not json",
        "[]",
        "{}",
        r#"{"account":"alice"}"#,
        r#"{"account":"","context_uri":null,"track":{"uri":"spotify:track:a","name":"","artists":[],"album":"","cover_url":null,"duration_ms":1},"position_ms":0}"#,
        r#"{"account":"alice","context_uri":null,"track":{"uri":"https://x","name":"","artists":[],"album":"","cover_url":null,"duration_ms":1},"position_ms":0}"#,
    ] {
        assert_eq!(parse(Some(raw), "alice"), None, "{raw}");
    }
}

#[test]
fn a_row_without_the_modes_reads_them_off_and_keeps_the_position_inside_the_track() {
    let raw = r#"{"account":"alice","context_uri":"not-a-uri","track":{"uri":"spotify:track:a","name":"A","artists":["B"],"album":"C","cover_url":null,"duration_ms":1000},"position_ms":5000}"#;
    let back = parse(Some(raw), "alice").unwrap();
    assert_eq!(back.position_ms, 1000);
    assert_eq!(back.context_uri, None);
    assert!(!back.shuffle && !back.repeat_context && !back.repeat_track);
    assert_eq!(back.saved_at_ms, 0);
}

#[test]
fn a_position_moves_on_only_while_playing_and_stops_at_the_end() {
    let ten = Duration::from_secs(10);
    assert_eq!(advanced(5_000, 200_000, false, ten), 5_000);
    assert_eq!(advanced(5_000, 200_000, true, ten), 15_000);
    assert_eq!(advanced(195_000, 200_000, true, ten), 200_000);
    // An unknown duration does not clamp.
    assert_eq!(advanced(5_000, 0, true, ten), 15_000);
}

#[test]
fn following_oculus_writes_on_a_change_and_every_fifteen_seconds_while_playing() {
    let t0 = Instant::now();
    let mut memory = Memory::default();
    let first = memory.follow(&playing(SONG, 1_000, 10), t0).unwrap();
    assert_eq!(first.account, "alice");
    assert_eq!(first.context_uri.as_deref(), Some(LIST));
    assert_eq!(first.position_ms, 1_000);

    // A change that is no new track, pause or mode writes nothing.
    let mut louder = playing(SONG, 1_000, 10);
    louder.volume = 40;
    assert_eq!(memory.follow(&louder, t0 + Duration::from_secs(1)), None);
    assert_eq!(memory.tick(t0 + Duration::from_secs(14)), None);
    let periodic = memory.tick(t0 + WRITE_EVERY).unwrap();
    assert_eq!(periodic.position_ms, 16_000);

    // A pause writes at once, where it stopped.
    let mut paused = playing(SONG, 20_000, 20);
    paused.playing = false;
    let at_pause = memory
        .follow(&paused, t0 + Duration::from_secs(19))
        .unwrap();
    assert_eq!(at_pause.position_ms, 20_000);
    // Paused, nothing more.
    assert_eq!(memory.tick(t0 + Duration::from_secs(120)), None);

    // A new track writes at once; its stamp still the old one, from the start.
    let mut next = paused.clone();
    next.track = Some(track(OTHER));
    let changed = memory.follow(&next, t0 + Duration::from_secs(121)).unwrap();
    assert_eq!(changed.track.uri, OTHER);
    assert_eq!(changed.position_ms, 0);
}

#[test]
fn another_device_taking_over_stops_what_oculus_played_where_it_was() {
    let t0 = Instant::now();
    let mut memory = Memory::default();
    memory.follow(&playing(SONG, 1_000, 10), t0);
    let mut gone = playing(SONG, 1_000, 10);
    gone.active = false;
    gone.playing = false;
    let frozen = memory.follow(&gone, t0 + Duration::from_secs(4)).unwrap();
    assert_eq!(frozen.position_ms, 5_000);
    assert_eq!(memory.tick(t0 + Duration::from_secs(60)), None);
    let shown = memory.shown(t0 + Duration::from_secs(60)).unwrap();
    assert_eq!(shown.position_ms, 5_000);
}

#[test]
fn a_drop_while_playing_resumes_playing_from_where_it_was() {
    let t0 = Instant::now();
    let mut memory = Memory::default();
    memory.follow(&playing(SONG, 30_000, 10), t0);
    let written = memory.lose(true, t0 + Duration::from_secs(3)).unwrap();
    assert_eq!(written.position_ms, 33_000);
    assert!(memory.resume_pending());
    // The reconnect takes a while; the position stays where the drop was.
    let (row, play) = memory.take_resume(t0 + Duration::from_secs(9)).unwrap();
    assert_eq!(row.position_ms, 33_000);
    assert_eq!(row.track.uri, SONG);
    assert!(play);
    assert!(!memory.resume_pending());
}

#[test]
fn a_drop_while_paused_resumes_paused_and_a_pause_meanwhile_holds_it_paused() {
    let t0 = Instant::now();
    let mut memory = Memory::default();
    let mut paused = playing(SONG, 30_000, 10);
    paused.playing = false;
    memory.follow(&paused, t0);
    memory.lose(true, t0);
    assert_eq!(memory.take_resume(t0).map(|(_, play)| play), Some(false));

    memory.follow(&playing(SONG, 30_000, 20), t0);
    memory.lose(true, t0);
    memory.hold_paused();
    assert_eq!(memory.take_resume(t0).map(|(_, play)| play), Some(false));
}

#[test]
fn a_recalled_row_shows_paused_and_is_not_written_back_until_it_changes() {
    let t0 = Instant::now();
    let mut memory = Memory::default();
    memory.recall(row("alice"), t0);
    assert_eq!(memory.account(), Some("alice"));
    assert_eq!(memory.tick(t0 + Duration::from_secs(60)), None);
    let shown = memory.shown(t0 + Duration::from_secs(60)).unwrap();
    assert_eq!(shown.position_ms, 61_000);
    // A stop writes it.
    assert!(memory.stop(t0 + Duration::from_secs(61)).is_some());
    memory.forget();
    assert!(memory.is_empty());
    assert_eq!(memory.stop(t0), None);
}

#[test]
fn a_spirc_that_said_it_disconnected_just_before_the_drop_still_resumes() {
    let t0 = Instant::now();
    let mut memory = Memory::default();
    memory.follow(&playing(SONG, 30_000, 10), t0);
    // The dying Spirc's `SessionDisconnected` lands before the link ends.
    let mut left = playing(SONG, 30_000, 10);
    left.active = false;
    left.playing = false;
    memory.follow(&left, t0 + Duration::from_secs(2));
    memory.lose(false, t0 + Duration::from_secs(5));
    assert!(memory.resume_plays());
    let (row, play) = memory.take_resume(t0 + Duration::from_secs(6)).unwrap();
    assert_eq!(row.position_ms, 32_000);
    assert!(play);

    // Long after, it was another device's turn: nothing resumes.
    memory.follow(&playing(SONG, 30_000, 20), t0);
    memory.follow(&left, t0 + Duration::from_secs(2));
    memory.lose(false, t0 + Duration::from_secs(2) + LEFT_RECENTLY * 2);
    assert!(!memory.resume_pending());
}

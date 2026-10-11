use std::time::Duration;

use librespot_core::SpotifyUri;
use librespot_playback::player::PlayerEvent;
use librespot_protocol::authentication::AuthenticationType;
use log::Level;

use super::credentials::Saved;
use super::events::apply;
use super::login::{callback_outcome, LoginError};
use super::logs::is_track_failure;
use super::session::{next_backoff, BACKOFF_MAX, BACKOFF_MIN};
use super::state::{from_percent, percent, Snapshot};
use super::track::cover_url;

#[test]
fn stored_credentials_round_trip_through_their_json() {
    let saved = Saved::stored("alice".into(), vec![0, 1, 2, 255], "dev-1".into());
    let text = saved.encode();
    assert!(text.contains("\"auth_data\":\"AAEC/w==\""), "{text}");
    let back = Saved::decode(&text).unwrap();
    assert_eq!(back, saved);
    assert_eq!(
        back.credentials.auth_type,
        AuthenticationType::AUTHENTICATION_STORED_SPOTIFY_CREDENTIALS
    );
}

#[test]
fn unreadable_or_incomplete_credentials_are_refused() {
    for text in [
        "",
        "not json",
        r#"{"username":"a","auth_data":"!!","device_id":"d"}"#,
        r#"{"username":"","auth_data":"AA==","device_id":"d"}"#,
        r#"{"username":"a","auth_data":"","device_id":"d"}"#,
        r#"{"username":"a","auth_data":"AA=="}"#,
    ] {
        assert!(Saved::decode(text).is_err(), "{text}");
    }
}

#[test]
fn the_cover_is_the_largest_up_to_640_on_spotifys_cdn() {
    let covers = [
        ("https://i.scdn.co/image/ab67616d00004851aa", 64),
        ("https://i.scdn.co/image/ab67616d0000b273bb", 640),
        ("https://i.scdn.co/image/ab67616d00001e02cc", 300),
    ];
    assert_eq!(
        cover_url(covers.into_iter()).as_deref(),
        Some("https://i.scdn.co/image/ab67616d0000b273bb")
    );
    // Another host keeps only the file id.
    assert_eq!(
        cover_url([("https://images.example.com/x/0a1b", 300)].into_iter()).as_deref(),
        Some("https://i.scdn.co/image/0a1b")
    );
    // Only bigger ones: the smallest.
    assert_eq!(
        cover_url(
            [
                ("https://i.scdn.co/image/ff", 1000),
                ("https://i.scdn.co/image/ee", 2000)
            ]
            .into_iter()
        )
        .as_deref(),
        Some("https://i.scdn.co/image/ff")
    );
    assert_eq!(
        cover_url([("https://i.scdn.co/image/", 64)].into_iter()),
        None
    );
    assert_eq!(cover_url(std::iter::empty()), None);
}

#[test]
fn the_callback_yields_the_code_only_with_the_right_state() {
    assert_eq!(
        callback_outcome("/login?code=abc&state=s1", "s1"),
        Some(Ok("abc".into()))
    );
    assert_eq!(callback_outcome("/favicon.ico", "s1"), None);
    assert!(matches!(
        callback_outcome("/login?code=abc&state=other", "s1"),
        Some(Err(LoginError::Failed(_)))
    ));
    assert!(matches!(
        callback_outcome("/login?error=access_denied&state=s1", "s1"),
        Some(Err(LoginError::Failed(why))) if why.contains("access_denied")
    ));
    assert!(matches!(
        callback_outcome("/login?state=s1", "s1"),
        Some(Err(LoginError::Failed(_)))
    ));
}

#[test]
fn a_busy_port_reads_as_the_snapshot_says_it() {
    assert_eq!(LoginError::PortBusy.to_string(), "Port 8898 is in use");
}

#[test]
fn volume_converts_both_ways_and_clamps() {
    assert_eq!(percent(0), 0);
    assert_eq!(percent(u16::MAX), 100);
    assert_eq!(percent(u16::MAX / 2), 50);
    for p in [0u8, 1, 33, 50, 99, 100] {
        assert_eq!(percent(from_percent(p)), p);
    }
    assert_eq!(from_percent(200), u16::MAX);
}

#[test]
fn backoff_doubles_from_two_seconds_to_a_minute() {
    let mut d = BACKOFF_MIN;
    let mut seen = vec![d.as_secs()];
    for _ in 0..7 {
        d = next_backoff(d);
        seen.push(d.as_secs());
    }
    assert_eq!(seen, [2, 4, 8, 16, 32, 60, 60, 60]);
    assert_eq!(BACKOFF_MAX, Duration::from_secs(60));
}

#[test]
fn a_missing_audio_key_is_a_track_failure() {
    assert!(is_track_failure(
        Level::Warn,
        "librespot_playback::player",
        "Unable to load key, continuing without decryption: x"
    ));
    assert!(is_track_failure(
        Level::Error,
        "librespot_audio::fetch",
        "anything"
    ));
    assert!(!is_track_failure(
        Level::Warn,
        "librespot_core::session",
        "Try another access point..."
    ));
}

#[test]
fn player_events_fold_into_the_snapshot() {
    let track_id = SpotifyUri::from_uri("spotify:track:4uLU6hMCjMI75M1A2tKUQC").unwrap();
    let mut s = Snapshot::default();
    apply(
        &mut s,
        PlayerEvent::Playing {
            play_request_id: 1,
            track_id: track_id.clone(),
            position_ms: 1500,
        },
    );
    assert!(s.playing);
    assert_eq!(s.position_ms, 1500);
    assert!(s.position_at_ms > 0);
    apply(
        &mut s,
        PlayerEvent::Paused {
            play_request_id: 1,
            track_id: track_id.clone(),
            position_ms: 2000,
        },
    );
    assert!(!s.playing);
    assert_eq!(s.position_ms, 2000);
    apply(&mut s, PlayerEvent::VolumeChanged { volume: u16::MAX });
    apply(&mut s, PlayerEvent::ShuffleChanged { shuffle: true });
    apply(
        &mut s,
        PlayerEvent::RepeatChanged {
            context: false,
            track: true,
        },
    );
    assert_eq!((s.volume, s.shuffle, s.repeat), (100, true, true));
    apply(
        &mut s,
        PlayerEvent::Unavailable {
            play_request_id: 1,
            track_id,
        },
    );
    assert!(s.error.as_deref().unwrap().contains("unavailable"));
}

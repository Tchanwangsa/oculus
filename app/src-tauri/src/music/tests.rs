use std::time::Duration;

use librespot_core::SpotifyUri;
use librespot_playback::player::PlayerEvent;
use librespot_protocol::authentication::AuthenticationType;
use log::Level;

use super::audio::{fallback_message, resolve, Choice};
use super::control::{step, At, Need, Step, QUEUE_REFUSAL};
use super::credentials::Saved;
use super::devices::{connect_kind, from_cluster, with_this};
use super::duck::Duck;
use super::events::apply;
use super::login::{callback_outcome, LoginError};
use super::logs::is_track_failure;
use super::queue::{find, nth, without};
use super::session::{next_backoff, BACKOFF_MAX, BACKOFF_MIN};
use super::startup::{parse, Prefs};
use super::state::{
    apply_cluster, from_percent, percent, up_next, NextTrack, Snapshot, INITIAL_VOLUME,
};
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
fn a_seek_with_nothing_loaded_is_not_a_track_failure() {
    assert!(!is_track_failure(
        Level::Error,
        "librespot_playback::player",
        "Player::seek called from invalid state: Stopped"
    ));
    assert!(!is_track_failure(
        Level::Error,
        "librespot_playback::player",
        "Player::play called from invalid state: Stopped"
    ));
    // A load the player cannot start still is.
    assert!(is_track_failure(
        Level::Error,
        "librespot_playback::player",
        "Player::handle_command_load called from invalid state: Invalid"
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

#[test]
fn only_an_explicit_enabled_setting_connects_at_startup() {
    assert!(parse(Some(r#"{"enabled":true}"#)).enabled);
    for raw in [
        None,
        Some(""),
        Some("true"),
        Some("{}"),
        Some(r#"{"enabled":false}"#),
        Some(r#"{"enabled":"yes"}"#),
    ] {
        assert!(!parse(raw).enabled, "{raw:?}");
    }
}

#[test]
fn the_settings_row_carries_the_output_and_ignores_unknown_keys() {
    assert_eq!(
        parse(Some(r#"{"enabled":true,"output":"AirPods","later":1}"#)),
        Prefs {
            enabled: true,
            output: Some("AirPods".into()),
        }
    );
    assert_eq!(
        parse(Some(r#"{"enabled":false,"output":null}"#)).output,
        None
    );
    assert_eq!(
        parse(Some(r#"{"output":"AirPods"}"#)).output.as_deref(),
        Some("AirPods")
    );
}

#[test]
fn a_new_device_starts_at_full_volume() {
    assert_eq!(INITIAL_VOLUME, 100);
    assert_eq!(Snapshot::default().volume, 100);
    assert!(!Snapshot::default().active);
}

#[test]
fn a_command_goes_here_to_the_other_device_or_pulls_by_where_playback_is() {
    use Need::*;
    let all = [Pull, Control, NeverPull, Load, Queue, IfActive];
    for need in all {
        assert_eq!(step(At::Here, need), Step::Send, "{need:?}");
    }
    // Another device playing is controlled where it is, except by "Play
    // here" and by what must never touch it (the exclusive rule, a duck).
    let elsewhere: Vec<Step> = all.iter().map(|n| step(At::Elsewhere, *n)).collect();
    assert_eq!(
        elsewhere,
        [
            Step::PullThenSend,
            Step::Remote,
            Step::Remote,
            Step::Remote,
            Step::Remote,
            Step::Skip
        ]
    );
    let nowhere: Vec<Step> = all.iter().map(|n| step(At::Nowhere, *n)).collect();
    assert_eq!(
        nowhere,
        [
            Step::PullThenSend,
            Step::PullThenSend,
            Step::Skip,
            Step::ActivateThenSend,
            Step::Refuse,
            Step::Skip
        ]
    );
    assert_eq!(QUEUE_REFUSAL, "Play something first, then add to the queue");
}

#[test]
fn becoming_active_and_inactive_follow_the_spircs_session_events() {
    let mut s = Snapshot {
        connected: true,
        playing_on: Some("Phone".into()),
        remote: Some(super::remote::RemotePlayback {
            device_name: "Phone".into(),
            track: None,
            position_ms: 0,
            position_at_ms: 0,
            playing: true,
            volume: None,
            shuffle: false,
            repeat: false,
            can_prev: true,
            uri: T1.into(),
        }),
        ..Snapshot::default()
    };
    apply(
        &mut s,
        PlayerEvent::SessionConnected {
            connection_id: "c".into(),
            user_name: "u".into(),
        },
    );
    assert!(s.active && s.connected);
    assert_eq!(s.playing_on, None);
    assert_eq!(s.remote, None);
    s.playing = true;
    apply(
        &mut s,
        PlayerEvent::SessionDisconnected {
            connection_id: "c".into(),
            user_name: "u".into(),
        },
    );
    // Another device took over; the session is still up.
    assert!(!s.active && !s.playing && s.connected);
}

#[test]
fn a_chosen_output_binds_when_present_and_falls_back_when_missing() {
    let present = ["MacBook Pro Speakers".to_string(), "AirPods".to_string()];
    assert_eq!(resolve(None, &present), (None, false));
    assert_eq!(
        resolve(Some("AirPods"), &present),
        (Some("AirPods".into()), false)
    );
    assert_eq!(resolve(Some("HomePod"), &present), (None, true));
    assert_eq!(
        fallback_message("HomePod"),
        "HomePod is not available; playing on the system default"
    );
}

#[test]
fn the_output_choice_bumps_its_generation_only_on_a_change() {
    let c = Choice::default();
    assert_eq!(c.get(), (None, 0));
    assert!(c.set(Some("AirPods".into())));
    assert!(!c.set(Some("AirPods".into())));
    assert_eq!(c.get(), (Some("AirPods".into()), 1));
    // A blank name is the system default.
    assert!(c.set(Some("  ".into())));
    assert_eq!(c.get(), (None, 2));
}

#[test]
fn device_types_map_to_the_pickers_kinds() {
    assert_eq!(connect_kind(1), "computer");
    assert_eq!(connect_kind(3), "phone");
    assert_eq!(connect_kind(4), "speaker");
    assert_eq!(connect_kind(5), "tv");
    assert_eq!(connect_kind(99), "other");
}

#[test]
fn before_the_first_push_the_list_is_this_device_alone() {
    let s = Snapshot {
        active: true,
        volume: 40,
        ..Snapshot::default()
    };
    let list = with_this(Vec::new(), "me", &s);
    assert_eq!(list.len(), 1);
    assert!(list[0].is_this && list[0].active);
    assert_eq!(
        (list[0].id.as_str(), list[0].name.as_str(), list[0].volume),
        ("me", "Oculus", Some(40))
    );
}

#[test]
fn a_pushed_cluster_lists_devices_this_one_first_and_names_the_other_active_one() {
    use librespot_protocol::connect::{Cluster, DeviceInfo};
    use librespot_protocol::devices::DeviceType;
    let info = |name: &str, kind: DeviceType, volume: u32, offline: bool| DeviceInfo {
        name: name.into(),
        device_type: kind.into(),
        volume,
        is_offline: offline,
        ..DeviceInfo::default()
    };
    let mut cluster = Cluster {
        active_device_id: "phone".into(),
        ..Cluster::default()
    };
    cluster.device.insert(
        "phone".into(),
        info("Pixel", DeviceType::SMARTPHONE, 65535, false),
    );
    cluster
        .device
        .insert("me".into(), info("Oculus", DeviceType::COMPUTER, 0, false));
    cluster
        .device
        .insert("tv".into(), info("Old TV", DeviceType::TV, 0, true));
    let c = from_cluster(&cluster, "me");
    let names: Vec<_> = c.devices.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["Oculus", "Pixel"]);
    assert!(c.devices[0].is_this && !c.devices[0].active);
    assert_eq!(
        (c.devices[1].kind, c.devices[1].volume),
        ("phone", Some(100))
    );
    assert_eq!(c.playing_on().as_deref(), Some("Pixel"));
    cluster.active_device_id = "me".into();
    assert_eq!(from_cluster(&cluster, "me").playing_on(), None);
}

#[test]
fn a_duck_lowers_from_the_kept_volume_and_the_unduck_restores_it() {
    let mut d = Duck::default();
    assert_eq!(d.unduck(), None);
    assert_eq!(d.duck(80, 25), 20);
    // Its own echo keeps it.
    d.seen(20);
    d.seen(21);
    assert!(d.is_held());
    // A second duck is a share of 80, not of 20, and keeps 80.
    assert_eq!(d.duck(20, 50), 40);
    assert_eq!(d.unduck(), Some(80));
    assert!(!d.is_held());
    assert_eq!(d.unduck(), None);
    assert_eq!(d.duck(60, 200), 60);
}

#[test]
fn a_volume_the_user_sets_while_ducked_wins() {
    let mut d = Duck::default();
    d.duck(80, 25);
    d.seen(55);
    assert_eq!(d.unduck(), None);
    d.duck(80, 25);
    d.forget();
    assert_eq!(d.unduck(), None);
}

fn next(uri: &str, queued: bool) -> NextTrack {
    NextTrack {
        uri: uri.into(),
        queued,
    }
}

const T1: &str = "spotify:track:4uLU6hMCjMI75M1A2tKUQC";
const T2: &str = "spotify:track:7ouMYWpwJ422jRcDASZB7P";
const T3: &str = "spotify:track:0eGsygTp906u18L0Oimnem";

#[test]
fn up_next_keeps_every_track_only_and_marks_queued_ones() {
    let listed = [
        (T1, "queue"),
        ("spotify:delimiter", "context"),
        ("spotify:episode:abc", "context"),
        ("spotify:local:a:b:c:1", "context"),
        (T2, "context"),
    ];
    assert_eq!(up_next(listed), [next(T1, true), next(T2, false)]);
    // librespot lists up to 80; none is cut.
    let many: Vec<(&str, &str)> = (0..80).map(|_| (T3, "context")).collect();
    assert_eq!(up_next(many).len(), 80);
}

#[test]
fn a_queue_event_sets_up_next_and_whether_previous_goes_back() {
    use librespot_playback::player::QueueTrack;
    let queued = |uri: &str, provider: &str| QueueTrack {
        uri: uri.into(),
        provider: provider.into(),
    };
    let set_queue = |next_tracks, prev_tracks| PlayerEvent::SetQueue {
        context_uri: "spotify:playlist:x".into(),
        current_track: None,
        next_tracks,
        prev_tracks,
    };
    let mut s = Snapshot::default();
    apply(
        &mut s,
        set_queue(
            vec![
                queued(T1, "queue"),
                queued(T2, "context"),
                queued(T3, "context"),
            ],
            vec![queued("spotify:delimiter", "context")],
        ),
    );
    assert_eq!(
        s.up_next,
        [next(T1, true), next(T2, false), next(T3, false)]
    );
    assert_eq!(s.context_uri.as_deref(), Some("spotify:playlist:x"));
    // A delimiter is not a track to go back to.
    assert!(!s.can_prev);

    // A track starting changes nothing up next; the Spirc's next event does.
    apply(
        &mut s,
        set_queue(vec![queued(T3, "context")], vec![queued(T2, "context")]),
    );
    assert_eq!(s.up_next, [next(T3, false)]);
    assert!(s.can_prev);

    // With none behind it, previous goes back only round a repeated context.
    apply(&mut s, set_queue(vec![], vec![]));
    assert!(!s.can_prev);
    apply(
        &mut s,
        PlayerEvent::RepeatChanged {
            context: false,
            track: true,
        },
    );
    assert!(s.repeat && !s.can_prev);
    apply(
        &mut s,
        PlayerEvent::RepeatChanged {
            context: true,
            track: false,
        },
    );
    assert!(s.can_prev);

    apply(
        &mut s,
        PlayerEvent::RepeatChanged {
            context: false,
            track: false,
        },
    );
    apply(&mut s, set_queue(vec![], vec![queued(T1, "context")]));
    s.set_unlinked();
    assert!(s.up_next.is_empty());
    assert_eq!(s.context_uri, None);
    assert!(!s.can_prev);
}

#[test]
fn a_pushed_cluster_carries_what_the_active_device_plays_next() {
    use librespot_protocol::connect::Cluster;
    use librespot_protocol::player::{PlayerState, ProvidedTrack};
    let track = |uri: &str, provider: &str| ProvidedTrack {
        uri: uri.into(),
        provider: provider.into(),
        ..ProvidedTrack::default()
    };
    let mut cluster = Cluster {
        active_device_id: "phone".into(),
        ..Cluster::default()
    };
    // No player state: nothing known, so the snapshot's list is kept.
    assert_eq!(from_cluster(&cluster, "me").up_next, None);
    cluster.player_state = Some(PlayerState {
        context_uri: "spotify:album:a".into(),
        next_tracks: vec![track(T1, "queue"), track(T2, "context")],
        prev_tracks: vec![track(T3, "context")],
        queue_revision: "42".into(),
        ..PlayerState::default()
    })
    .into();
    let pushed = from_cluster(&cluster, "me");
    assert_eq!(pushed.up_next, Some(vec![next(T1, true), next(T2, false)]));
    let queue = pushed.queue.as_ref().unwrap();
    assert_eq!(
        (queue.next.len(), queue.prev.len(), queue.revision.as_str()),
        (2, 1, "42")
    );
    let mut s = Snapshot::default();
    apply_cluster(&mut s, &mut pushed.clone());
    assert_eq!(s.context_uri.as_deref(), Some("spotify:album:a"));

    // A push without a player state keeps the last queue of the same device.
    let mut bare = cluster.clone();
    bare.player_state.clear();
    let mut later = from_cluster(&bare, "me");
    later.keep_queue(Some(pushed.clone()));
    assert_eq!(later.queue, pushed.queue);
    apply_cluster(&mut s, &mut later);
    assert_eq!(s.context_uri.as_deref(), Some("spotify:album:a"));
    // Not another device's.
    bare.active_device_id = "speaker".into();
    let mut other = from_cluster(&bare, "me");
    other.keep_queue(Some(pushed.clone()));
    assert_eq!(other.queue, None);

    // Nothing active: nothing plays next, from no context.
    cluster.active_device_id.clear();
    let mut idle = from_cluster(&cluster, "me");
    assert_eq!(idle.up_next, Some(vec![]));
    idle.keep_queue(Some(pushed));
    assert_eq!(idle.queue, None);
    apply_cluster(&mut s, &mut idle);
    assert_eq!(s.context_uri, None);
}

fn provided(uri: &str, uid: &str) -> librespot_protocol::player::ProvidedTrack {
    librespot_protocol::player::ProvidedTrack {
        uri: uri.into(),
        uid: uid.into(),
        provider: "context".into(),
        ..Default::default()
    }
}

#[test]
fn an_up_next_entry_is_found_by_its_place_among_the_listed_tracks() {
    let next = [
        provided(T1, "a"),
        provided("spotify:episode:e", "b"),
        provided(T2, "c"),
        provided("spotify:delimiter", "d"),
        provided(T1, "e"),
    ];
    // up_next is T1, T2, T1: the episode and the delimiter are not shown.
    assert_eq!(find(&next, T1, 0), Some(0));
    assert_eq!(find(&next, T2, 1), Some(2));
    // A duplicate URI: the entry at that place, not the first.
    assert_eq!(find(&next, T1, 2), Some(4));
    // The list moved: the first entry with that URI.
    assert_eq!(find(&next, T2, 0), Some(2));
    assert_eq!(find(&next, T2, 9), Some(2));
    assert_eq!(find(&next, T3, 0), None);
    assert_eq!(find(&[], T1, 0), None);
}

#[test]
fn a_removal_keeps_every_other_entry_episodes_and_delimiters_included() {
    let next = [
        provided(T1, "a"),
        provided("spotify:episode:e", "b"),
        provided(T2, "c"),
        provided(T1, "d"),
    ];
    let at = find(&next, T1, 2).unwrap();
    let uids: Vec<String> = without(&next, at).into_iter().map(|t| t.uid).collect();
    assert_eq!(uids, ["a", "b", "c"]);
    let uids: Vec<String> = without(&next, 0).into_iter().map(|t| t.uid).collect();
    assert_eq!(uids, ["b", "c", "d"]);
}

#[test]
fn oculus_names_an_up_next_entry_by_its_uri_and_which_of_its_tracks_it_is() {
    let up = [
        next(T1, true),
        next(T2, false),
        next(T1, false),
        next(T1, false),
    ];
    assert_eq!(nth(&up, T1, 0), 0);
    assert_eq!(nth(&up, T2, 1), 0);
    assert_eq!(nth(&up, T1, 2), 1);
    assert_eq!(nth(&up, T1, 3), 2);
    // The entry moved or the list ended: the first track with that URI.
    assert_eq!(nth(&up, T2, 3), 0);
    assert_eq!(nth(&up, T1, 9), 0);
}

use librespot_protocol::connect::{Capabilities, Cluster as ClusterProto, DeviceInfo};
use librespot_protocol::player::{ContextPlayerOptions, PlayerState, ProvidedTrack, Restrictions};
use serde_json::json;

use super::command::{body, path, ListedTrack};
use super::playback::{from_cluster, with_row};
use super::{refused, RemoteCommand, RemotePlayback};
use crate::music::devices::from_cluster as devices_from_cluster;
use crate::music::library::{LibraryError, TrackRow};
use crate::music::state::{apply_cluster, Snapshot};

const T1: &str = "spotify:track:4uLU6hMCjMI75M1A2tKUQC";
const T2: &str = "spotify:track:7ouMYWpwJ422jRcDASZB7P";
const PLAYLIST: &str = "spotify:playlist:37i9dQZF1DXcBWIGoYBM5M";

fn command(endpoint: &str, extra: serde_json::Value) -> serde_json::Value {
    let mut fields = json!({ "endpoint": endpoint, "logging_params": {} });
    for (k, v) in extra.as_object().unwrap() {
        fields[k] = v.clone();
    }
    json!({ "command": fields })
}

#[test]
fn player_commands_post_to_the_command_endpoint_and_volume_puts_its_own() {
    assert_eq!(
        path(&RemoteCommand::Pause, "me", "phone"),
        "/connect-state/v1/player/command/from/me/to/phone"
    );
    assert_eq!(
        path(&RemoteCommand::Volume(50), "me", "phone"),
        "/connect-state/v1/connect/volume/from/me/to/phone"
    );
}

#[test]
fn bare_commands_carry_only_the_endpoint_and_empty_logging_params() {
    for (cmd, endpoint) in [
        (RemoteCommand::Pause, "pause"),
        (RemoteCommand::Resume, "resume"),
        (RemoteCommand::SkipNext, "skip_next"),
        (RemoteCommand::SkipPrev, "skip_prev"),
    ] {
        assert_eq!(body(&cmd), command(endpoint, json!({})), "{endpoint}");
    }
}

#[test]
fn valued_commands_carry_what_librespots_receiver_reads() {
    assert_eq!(
        body(&RemoteCommand::SeekTo(61_000)),
        command("seek_to", json!({ "value": 61_000, "position": 61_000 }))
    );
    assert_eq!(
        body(&RemoteCommand::Shuffle(true)),
        command("set_shuffling_context", json!({ "value": true }))
    );
    assert_eq!(
        body(&RemoteCommand::RepeatContext(false)),
        command("set_repeating_context", json!({ "value": false }))
    );
    assert_eq!(
        body(&RemoteCommand::RepeatTrack(false)),
        command("set_repeating_track", json!({ "value": false }))
    );
    assert_eq!(
        body(&RemoteCommand::AddToQueue(T1.into())),
        command(
            "add_to_queue",
            json!({ "track": {
                "uri": T1,
                "provider": "queue",
                "metadata": { "is_queued": "true" },
            }})
        )
    );
    assert_eq!(
        body(&RemoteCommand::Volume(100)),
        json!({ "volume": 65535 })
    );
    assert_eq!(body(&RemoteCommand::Volume(0)), json!({ "volume": 0 }));
}

fn listed(uri: &str, uid: &str, provider: &str, metadata: &[(&str, &str)]) -> ListedTrack {
    ListedTrack {
        uri: uri.into(),
        uid: uid.into(),
        provider: provider.into(),
        metadata: metadata
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

#[test]
fn a_skip_to_names_the_track_with_its_uid_and_provider_when_known() {
    assert_eq!(
        body(&RemoteCommand::SkipTo(listed(
            T1,
            "q3",
            "queue",
            &[("is_queued", "true")]
        ))),
        command(
            "skip_next",
            json!({ "track": { "uri": T1, "uid": "q3", "provider": "queue" } })
        )
    );
    assert_eq!(
        body(&RemoteCommand::SkipTo(listed(T1, "", "", &[]))),
        command("skip_next", json!({ "track": { "uri": T1 } }))
    );
    assert_eq!(
        path(&RemoteCommand::SkipTo(listed(T1, "", "", &[])), "me", "me"),
        "/connect-state/v1/player/command/from/me/to/me"
    );
}

#[test]
fn a_set_queue_lists_next_and_previous_tracks_whole_with_the_revision() {
    let next = vec![
        listed(T1, "q0", "queue", &[("is_queued", "true")]),
        listed("spotify:delimiter", "", "context", &[("hidden", "true")]),
    ];
    let prev = vec![listed(T2, "c9", "context", &[])];
    assert_eq!(
        body(&RemoteCommand::SetQueue {
            next,
            prev,
            revision: "1234".into(),
        }),
        command(
            "set_queue",
            json!({
                "next_tracks": [
                    {
                        "uri": T1,
                        "uid": "q0",
                        "provider": "queue",
                        "metadata": { "is_queued": "true" },
                    },
                    {
                        "uri": "spotify:delimiter",
                        "uid": "",
                        "provider": "context",
                        "metadata": { "hidden": "true" },
                    },
                ],
                "prev_tracks": [
                    { "uri": T2, "uid": "c9", "provider": "context", "metadata": {} },
                ],
                "queue_revision": "1234",
            })
        )
    );
}

#[test]
fn a_play_names_the_context_and_skips_to_a_track_only_when_given_one() {
    let context = json!({ "uri": PLAYLIST, "url": format!("context://{PLAYLIST}") });
    let origin = json!({ "feature_identifier": "oculus" });
    assert_eq!(
        body(&RemoteCommand::Play {
            context: PLAYLIST.into(),
            track: Some(T1.into()),
            shuffle: None,
        }),
        command(
            "play",
            json!({
                "context": context,
                "play_origin": origin,
                "options": { "skip_to": { "track_uri": T1 } },
            })
        )
    );
    assert_eq!(
        body(&RemoteCommand::Play {
            context: PLAYLIST.into(),
            track: None,
            shuffle: None,
        }),
        command(
            "play",
            json!({ "context": context, "play_origin": origin, "options": {} })
        )
    );
    // Play and Shuffle play on a playlist or album state shuffle either way.
    for shuffle in [true, false] {
        assert_eq!(
            body(&RemoteCommand::Play {
                context: PLAYLIST.into(),
                track: None,
                shuffle: Some(shuffle),
            }),
            command(
                "play",
                json!({
                    "context": context,
                    "play_origin": origin,
                    "options": { "player_options_override": { "shuffling_context": shuffle } },
                })
            )
        );
    }
}

#[test]
fn a_refused_command_names_the_device_and_status() {
    assert_eq!(
        refused("iPhone", &LibraryError::Status(400)),
        "Spotify refused the command on iPhone (400)"
    );
    assert_eq!(
        refused("iPhone", &LibraryError::Forbidden),
        "Spotify refused the command on iPhone (403)"
    );
    assert_eq!(
        refused("iPhone", &LibraryError::RateLimited(5)),
        "Spotify is rate-limiting Oculus; try again in 5 s"
    );
}

fn device(name: &str, volume: u32) -> DeviceInfo {
    DeviceInfo {
        name: name.into(),
        volume,
        ..DeviceInfo::default()
    }
}

fn phone_playing() -> ClusterProto {
    let mut cluster = ClusterProto {
        active_device_id: "phone".into(),
        ..ClusterProto::default()
    };
    cluster
        .device
        .insert("phone".into(), device("iPhone", 32768));
    cluster.device.insert("me".into(), device("Oculus", 65535));
    let mut track = ProvidedTrack {
        uri: T1.into(),
        ..ProvidedTrack::default()
    };
    track.metadata.insert("title".into(), "Song".into());
    track.metadata.insert("album_title".into(), "Album".into());
    track.metadata.insert(
        "image_url".into(),
        "spotify:image:ab67616d00001e02aa".into(),
    );
    track.metadata.insert(
        "image_large_url".into(),
        "spotify:image:ab67616d0000b273bb".into(),
    );
    cluster.player_state = Some(PlayerState {
        timestamp: 1_700_000_000_000,
        position_as_of_timestamp: 42_000,
        duration: 200_000,
        is_playing: true,
        is_paused: false,
        track: Some(track).into(),
        options: Some(ContextPlayerOptions {
            shuffling_context: true,
            repeating_track: true,
            ..ContextPlayerOptions::default()
        })
        .into(),
        ..PlayerState::default()
    })
    .into();
    cluster
}

#[test]
fn another_devices_player_state_becomes_the_remote_playback() {
    let remote = from_cluster(&phone_playing(), "me").unwrap();
    assert_eq!(remote.device_name, "iPhone");
    assert_eq!(remote.uri, T1);
    assert_eq!(
        (remote.position_ms, remote.position_at_ms),
        (42_000, 1_700_000_000_000)
    );
    assert!(remote.playing && remote.shuffle && remote.repeat);
    assert_eq!(remote.volume, Some(50));
    let track = remote.track.unwrap();
    assert_eq!(
        (track.name.as_str(), track.album.as_str()),
        ("Song", "Album")
    );
    assert!(track.artists.is_empty());
    // The 640 px cover over the 300 px one.
    assert_eq!(
        track.cover_url.as_deref(),
        Some("https://i.scdn.co/image/ab67616d0000b273bb")
    );
    assert_eq!(track.duration_ms, 200_000);
}

#[test]
fn paused_without_metadata_or_volume_control() {
    let mut cluster = phone_playing();
    let player = cluster.player_state.as_mut().unwrap();
    player.is_paused = true;
    player.track.as_mut().unwrap().metadata.clear();
    cluster.device.get_mut("phone").unwrap().capabilities = Some(Capabilities {
        disable_volume: true,
        ..Capabilities::default()
    })
    .into();
    let remote = from_cluster(&cluster, "me").unwrap();
    assert!(!remote.playing);
    assert_eq!(remote.track, None);
    assert_eq!(remote.volume, None);
}

#[test]
fn previous_is_allowed_unless_the_player_state_restricts_it() {
    let mut cluster = phone_playing();
    // No restrictions message: nothing forbids it.
    assert!(from_cluster(&cluster, "me").unwrap().can_prev);
    let player = cluster.player_state.as_mut().unwrap();
    player.restrictions = Some(Restrictions::default()).into();
    assert!(from_cluster(&cluster, "me").unwrap().can_prev);
    let player = cluster.player_state.as_mut().unwrap();
    player.restrictions = Some(Restrictions {
        disallow_skipping_prev_reasons: vec!["no_prev_track".into()],
        ..Restrictions::default()
    })
    .into();
    assert!(!from_cluster(&cluster, "me").unwrap().can_prev);
}

#[test]
fn there_is_no_remote_playback_here_with_nothing_active_or_without_a_player_state() {
    let mut cluster = phone_playing();
    cluster.active_device_id = "me".into();
    assert_eq!(from_cluster(&cluster, "me"), None);
    cluster.active_device_id.clear();
    assert_eq!(from_cluster(&cluster, "me"), None);
    let mut cluster = phone_playing();
    cluster.player_state.clear();
    assert_eq!(from_cluster(&cluster, "me"), None);
}

fn row(uri: &str) -> TrackRow {
    TrackRow {
        uri: uri.into(),
        name: "Song (Remastered)".into(),
        artists: vec!["Artist".into()],
        album: "Album".into(),
        image_url: Some("https://i.scdn.co/image/ab67616d00001e02aa".into()),
        duration_ms: 199_000,
        album_key: String::new(),
    }
}

#[test]
fn the_library_row_names_the_track_keeping_the_metadatas_cover_and_duration() {
    let mut remote = from_cluster(&phone_playing(), "me").unwrap();
    with_row(&mut remote, &row(T1));
    let track = remote.track.unwrap();
    assert_eq!(track.name, "Song (Remastered)");
    assert_eq!(track.artists, ["Artist"]);
    assert_eq!(
        track.cover_url.as_deref(),
        Some("https://i.scdn.co/image/ab67616d0000b273bb")
    );
    assert_eq!(track.duration_ms, 200_000);

    // Without metadata the row gives all of it.
    let mut remote = RemotePlayback {
        track: None,
        ..from_cluster(&phone_playing(), "me").unwrap()
    };
    with_row(&mut remote, &row(T1));
    let track = remote.track.unwrap();
    assert_eq!(
        track.cover_url.as_deref(),
        Some("https://i.scdn.co/image/ab67616d00001e02aa")
    );
    assert_eq!(track.duration_ms, 199_000);
}

#[test]
fn a_push_sets_the_remote_and_keeps_it_through_one_without_player_state() {
    let mut s = Snapshot::default();
    apply_cluster(&mut s, &mut devices_from_cluster(&phone_playing(), "me"));
    assert_eq!(s.playing_on.as_deref(), Some("iPhone"));
    assert_eq!(s.remote.as_ref().map(|r| r.uri.as_str()), Some(T1));

    let mut bare = phone_playing();
    bare.player_state.clear();
    apply_cluster(&mut s, &mut devices_from_cluster(&bare, "me"));
    assert!(s.remote.is_some());

    let mut here = phone_playing();
    here.active_device_id = "me".into();
    apply_cluster(&mut s, &mut devices_from_cluster(&here, "me"));
    assert_eq!((s.playing_on.as_ref(), s.remote.as_ref()), (None, None));
    // Local playback is never touched by a push.
    assert_eq!(s.track, None);
    assert!(!s.playing);
}

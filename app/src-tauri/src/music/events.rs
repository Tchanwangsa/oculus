//! The player's events, folded into the snapshot of run `id`. The channel
//! closes when the player is dropped, which ends this task.

use librespot_playback::player::{PlayerEvent, PlayerEventChannel};
use tauri::AppHandle;

use super::state::{self, context, has_prev, percent, up_next, Snapshot};
use super::track;

pub(super) async fn run(app: AppHandle, id: u64, mut rx: PlayerEventChannel) {
    while let Some(event) = rx.recv().await {
        if let PlayerEvent::Unavailable { track_id, .. } = &event {
            elog!("[music] track unavailable: {}", track_id.to_uri());
        }
        if let PlayerEvent::VolumeChanged { volume } = &event {
            state::volume_seen(&app, percent(*volume));
        }
        state::update_run(&app, id, |s| apply(s, event));
    }
}

/// One event's effect. Positions are stamped with the time they were true.
pub(super) fn apply(s: &mut Snapshot, event: PlayerEvent) {
    match event {
        PlayerEvent::TrackChanged { audio_item } => {
            s.track = Some(track::from_item(&audio_item));
        }
        // The Spirc sends one whenever the next or previous tracks change:
        // a load, the queue, shuffle, repeat, and every move on or back.
        PlayerEvent::SetQueue {
            context_uri,
            next_tracks,
            prev_tracks,
            ..
        } => {
            s.up_next = up_next(
                next_tracks
                    .iter()
                    .map(|t| (t.uri.as_str(), t.provider.as_str())),
            );
            s.context_uri = context(&context_uri);
            s.set_has_prev(has_prev(prev_tracks.iter().map(|t| t.uri.as_str())));
        }
        PlayerEvent::Playing { position_ms, .. } => {
            s.playing = true;
            s.set_position(position_ms);
        }
        PlayerEvent::Paused { position_ms, .. } => {
            s.playing = false;
            s.set_position(position_ms);
        }
        PlayerEvent::Loading { position_ms, .. }
        | PlayerEvent::Seeked { position_ms, .. }
        | PlayerEvent::PositionCorrection { position_ms, .. }
        | PlayerEvent::PositionChanged { position_ms, .. } => s.set_position(position_ms),
        PlayerEvent::Stopped { .. } => {
            s.playing = false;
        }
        PlayerEvent::Unavailable { track_id, .. } => {
            s.error = Some(format!(
                "Spotify could not play {}: it is unavailable",
                track_id.to_uri()
            ));
        }
        PlayerEvent::VolumeChanged { volume } => s.volume = percent(volume),
        PlayerEvent::ShuffleChanged { shuffle } => s.shuffle = shuffle,
        PlayerEvent::RepeatChanged { context, track } => s.set_repeat(context, track),
        // The Spirc's own word on being the active Connect device: it emits
        // these on activating and on another device taking over, while the
        // session itself stays up.
        PlayerEvent::SessionConnected { .. } => {
            s.active = true;
            s.playing_on = None;
            s.remote = None;
        }
        PlayerEvent::SessionDisconnected { .. } => {
            s.active = false;
            s.playing = false;
        }
        _ => {}
    }
}

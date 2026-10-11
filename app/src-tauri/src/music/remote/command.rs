//! The requests that command another Connect device, built as pure values so
//! a wrong shape is easy to see and fix. Player commands go to Spotify's
//! connect-state player command API, the one Spotify's own apps send to
//! remote-control each other; the body's shape is what librespot's receiver
//! reads (`core/src/dealer/protocol/request.rs`), where `logging_params` is a
//! required field. Volume has its own connect-state endpoint.

use std::collections::BTreeMap;

use librespot_protocol::player::ProvidedTrack;
use serde_json::{json, Value};

use crate::music::state::from_percent;

/// A track as the active device's player state lists it, with what
/// `skip_next` and `set_queue` send back (`queue.rs`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::music) struct ListedTrack {
    pub(in crate::music) uri: String,
    pub(in crate::music) uid: String,
    pub(in crate::music) provider: String,
    pub(in crate::music) metadata: BTreeMap<String, String>,
}

impl From<&ProvidedTrack> for ListedTrack {
    fn from(t: &ProvidedTrack) -> Self {
        Self {
            uri: t.uri.clone(),
            uid: t.uid.clone(),
            provider: t.provider.clone(),
            metadata: t
                .metadata
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }
}

impl ListedTrack {
    /// The whole track, as `set_queue` lists it.
    fn json(&self) -> Value {
        json!({
            "uri": self.uri,
            "uid": self.uid,
            "provider": self.provider,
            "metadata": self.metadata,
        })
    }
}

/// A command for the device playing elsewhere.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::music) enum RemoteCommand {
    Pause,
    Resume,
    SkipNext,
    SkipPrev,
    SeekTo(u32),
    Shuffle(bool),
    RepeatContext(bool),
    RepeatTrack(bool),
    /// A `spotify:track:` URI.
    AddToQueue(String),
    /// Moves on until this track, from the next tracks, is the current one.
    SkipTo(ListedTrack),
    /// Replaces the next and previous tracks; `revision` is the queue
    /// revision they were read at.
    SetQueue {
        next: Vec<ListedTrack>,
        prev: Vec<ListedTrack>,
        revision: String,
    },
    /// Plays `context` from its start, or from `track` inside it.
    Play {
        context: String,
        track: Option<String>,
    },
    /// 0–100.
    Volume(u8),
}

impl RemoteCommand {
    /// The player command's endpoint name; `volume` for volume, which has
    /// an endpoint of its own.
    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::SkipNext | Self::SkipTo(_) => "skip_next",
            Self::SkipPrev => "skip_prev",
            Self::SeekTo(_) => "seek_to",
            Self::Shuffle(_) => "set_shuffling_context",
            Self::RepeatContext(_) => "set_repeating_context",
            Self::RepeatTrack(_) => "set_repeating_track",
            Self::AddToQueue(_) => "add_to_queue",
            Self::SetQueue { .. } => "set_queue",
            Self::Play { .. } => "play",
            Self::Volume(_) => "volume",
        }
    }
}

/// The path, after the spclient base, from this device `from` to `to`.
pub(super) fn path(command: &RemoteCommand, from: &str, to: &str) -> String {
    match command {
        RemoteCommand::Volume(_) => format!("/connect-state/v1/connect/volume/from/{from}/to/{to}"),
        _ => format!("/connect-state/v1/player/command/from/{from}/to/{to}"),
    }
}

/// The JSON body: `{"volume": 0..=65535}` for volume, else
/// `{"command": {"endpoint": …, …, "logging_params": {}}}`.
pub(super) fn body(command: &RemoteCommand) -> Value {
    let mut fields = match command {
        RemoteCommand::Volume(percent) => return json!({ "volume": from_percent(*percent) }),
        RemoteCommand::SeekTo(ms) => json!({ "value": ms, "position": ms }),
        RemoteCommand::Shuffle(on)
        | RemoteCommand::RepeatContext(on)
        | RemoteCommand::RepeatTrack(on) => json!({ "value": on }),
        RemoteCommand::AddToQueue(uri) => json!({
            "track": {
                "uri": uri,
                "provider": "queue",
                "metadata": { "is_queued": "true" },
            },
        }),
        RemoteCommand::SkipTo(track) => {
            let mut named = json!({ "uri": track.uri });
            if !track.uid.is_empty() {
                named["uid"] = json!(track.uid);
            }
            if !track.provider.is_empty() {
                named["provider"] = json!(track.provider);
            }
            json!({ "track": named })
        }
        RemoteCommand::SetQueue {
            next,
            prev,
            revision,
        } => json!({
            "next_tracks": next.iter().map(ListedTrack::json).collect::<Vec<_>>(),
            "prev_tracks": prev.iter().map(ListedTrack::json).collect::<Vec<_>>(),
            "queue_revision": revision,
        }),
        RemoteCommand::Play { context, track } => {
            let mut options = json!({});
            if let Some(track) = track {
                options["skip_to"] = json!({ "track_uri": track });
            }
            json!({
                "context": { "uri": context, "url": format!("context://{context}") },
                "play_origin": { "feature_identifier": "oculus" },
                "options": options,
            })
        }
        RemoteCommand::Pause
        | RemoteCommand::Resume
        | RemoteCommand::SkipNext
        | RemoteCommand::SkipPrev => json!({}),
    };
    fields["endpoint"] = json!(command.label());
    fields["logging_params"] = json!({});
    json!({ "command": fields })
}

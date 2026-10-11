//! Skipping to a track up next and removing one, on whichever device plays.
//!
//! While Oculus plays, both go to the Spirc (`skip_to`, `remove_next`), which
//! the vendored librespot-connect adds (`vendor/librespot-connect`): they find
//! the track in its own queue and do nothing when it is not there. The entry
//! is named by URI and which of that URI's tracks it is (`nth`).
//!
//! Another device gets Spotify's remote commands (`remote/`): `skip_next`
//! naming the track, or `set_queue` with every next track but that one and
//! the previous tracks unchanged, since it replaces them too. Both are built
//! from that device's player state in the last cluster push (`Queue`).

use librespot_protocol::player::{PlayerState, ProvidedTrack};
use tauri::AppHandle;

use super::control::{self, Need, Route, QUEUE_REFUSAL};
use super::remote::{self, ListedTrack, RemoteCommand};
use super::state::{self, listed, NextTrack};

/// Why a skip or a removal found nothing to act on.
pub(super) const GONE: &str = "That track is no longer up next";
/// Why a skip or a removal was refused with nothing active anywhere.
const NOTHING_PLAYS: &str = "Play something first";

/// The active device's queue as the last cluster push listed it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Queue {
    pub(super) next: Vec<ProvidedTrack>,
    pub(super) prev: Vec<ProvidedTrack>,
    pub(super) revision: String,
    pub(super) context_uri: String,
}

impl Queue {
    pub(super) fn from_player(player: &PlayerState) -> Self {
        Self {
            next: player.next_tracks.clone(),
            prev: player.prev_tracks.clone(),
            revision: player.queue_revision.clone(),
            context_uri: player.context_uri.clone(),
        }
    }
}

/// The position in `next` of the snapshot's `up_next` entry at `index`,
/// which names `uri`: `up_next` is `next` with only the `listed` tracks, so
/// that entry, or, when the list has moved since the snapshot, the first
/// listed one with that URI.
pub(super) fn find(next: &[ProvidedTrack], uri: &str, index: usize) -> Option<usize> {
    let shown: Vec<usize> = next
        .iter()
        .enumerate()
        .filter(|(_, t)| listed(&t.uri))
        .map(|(at, _)| at)
        .collect();
    shown
        .get(index)
        .filter(|&&at| next[at].uri == uri)
        .or_else(|| shown.iter().find(|&&at| next[at].uri == uri))
        .copied()
}

/// `next` without the track at `at`, everything else (episodes and
/// delimiters too) kept in order.
pub(super) fn without(next: &[ProvidedTrack], at: usize) -> Vec<ListedTrack> {
    next.iter()
        .enumerate()
        .filter(|(i, _)| *i != at)
        .map(|(_, t)| ListedTrack::from(t))
        .collect()
}

/// Which track with `uri` the `up_next` entry at `index` is, counting from
/// 0: what the Spirc's `skip_to` and `remove_next` take. The first when the
/// entry no longer names `uri`.
pub(super) fn nth(up_next: &[NextTrack], uri: &str, index: usize) -> usize {
    match up_next.get(index) {
        Some(t) if t.uri == uri => up_next[..index].iter().filter(|t| t.uri == uri).count(),
        _ => 0,
    }
}

/// Plays the `up_next` entry at `index` (named `uri`) now, skipping what
/// comes before it.
pub(super) async fn skip_to(app: &AppHandle, uri: &str, index: usize) -> Result<(), String> {
    match target(app).await? {
        Target::Here => {
            let nth = nth(&state::snapshot(app).up_next, uri, index);
            state::with_spirc(app, |spirc| spirc.skip_to(uri.to_string(), nth))
        }
        Target::There(device, queue) => {
            let at = find(&queue.next, uri, index).ok_or(GONE)?;
            let track = ListedTrack::from(&queue.next[at]);
            remote::send(app, &device, RemoteCommand::SkipTo(track)).await
        }
    }
}

/// Takes the `up_next` entry at `index` (named `uri`) out of the queue.
pub(super) async fn remove(app: &AppHandle, uri: &str, index: usize) -> Result<(), String> {
    match target(app).await? {
        Target::Here => {
            let nth = nth(&state::snapshot(app).up_next, uri, index);
            state::with_spirc(app, |spirc| spirc.remove_next(uri.to_string(), nth))
        }
        Target::There(device, queue) => {
            let at = find(&queue.next, uri, index).ok_or(GONE)?;
            let command = RemoteCommand::SetQueue {
                next: without(&queue.next, at),
                prev: queue.prev.iter().map(ListedTrack::from).collect(),
                revision: queue.revision.clone(),
            };
            remote::send(app, &device, command).await
        }
    }
}

/// Where a skip or a removal goes.
enum Target {
    /// Oculus plays: the Spirc, which finds the track in its own queue.
    Here,
    /// Another device, with its queue from the last push.
    There(remote::Device, Queue),
}

async fn target(app: &AppHandle) -> Result<Target, String> {
    match control::route(app, Need::Queue).await {
        Ok(Route::Here) => Ok(Target::Here),
        Ok(Route::There(device)) => {
            let queue = state::cluster(app)
                .filter(|c| c.active_id == device.to)
                .and_then(|c| c.queue)
                .ok_or("Spotify has not listed the queue yet; try again in a moment")?;
            Ok(Target::There(device, queue))
        }
        Ok(Route::Skip) => Err(NOTHING_PLAYS.into()),
        Err(e) if e == QUEUE_REFUSAL => Err(NOTHING_PLAYS.into()),
        Err(e) => Err(e),
    }
}

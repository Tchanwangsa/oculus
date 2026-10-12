//! The account's Spotify Connect devices, and moving playback between them.
//!
//! - **The list is the Connect cluster** Spotify pushes over librespot's
//!   dealer (`hm://connect-state/v1/cluster`, which the Spirc also hears):
//!   free, and never rate-limited. Each session subscribes once, and the last
//!   push is kept until the connection drops. Spotify pushes only on a
//!   change, and the cluster the Spirc gets back when it announces the device
//!   stays inside the Spirc, so until the first push the list is this device
//!   alone. A push also carries the active device's next tracks and context,
//!   for the snapshot's `up_next` and `context_uri` and for skipping to or
//!   removing a track (`queue.rs`), and, when that device is another one,
//!   what it plays, for the snapshot's `remote` (`remote/playback.rs`).
//! - **A transfer** to Oculus itself is a pull (`control.rs`) then play; to
//!   another device it is librespot's `spclient().transfer(from, to)`, the
//!   request Spotify's own apps send.

use futures_util::StreamExt;
use librespot_core::dealer::protocol::Message;
use librespot_core::Session;
use librespot_protocol::connect::{Cluster as ClusterProto, ClusterUpdate};
use serde::Serialize;
use tauri::AppHandle;

use super::control;
use super::queue::Queue;
use super::remote::{self, RemotePlayback};
use super::state::{self, percent, up_next, NextTrack, DEVICE_NAME};

const CLUSTER_URI: &str = "hm://connect-state/v1/cluster";

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    /// computer, phone, tablet, speaker, tv, car or other.
    pub kind: &'static str,
    pub active: bool,
    /// 0–100, when the device reports one.
    pub volume: Option<u8>,
    /// This Oculus; its `id` is always this session's device id.
    pub is_this: bool,
}

/// The cluster as the last push left it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cluster {
    pub(super) active_id: String,
    pub(super) devices: Vec<Device>,
    /// What the active device plays next, when the push carried its player
    /// state; empty when no device is active.
    pub(super) up_next: Option<Vec<NextTrack>>,
    /// The active device's queue and context in full, for skipping to and
    /// removing a track (`queue.rs`); none when nothing is active, and
    /// carried over from the last push through one without a player state.
    pub(super) queue: Option<Queue>,
    /// What the active device plays when it is another one and the push
    /// carried its player state; moved into the snapshot on arrival.
    pub(super) remote: Option<RemotePlayback>,
}

impl Cluster {
    /// The active device's name when it is not this one.
    pub(super) fn playing_on(&self) -> Option<String> {
        self.devices
            .iter()
            .find(|d| d.active && !d.is_this)
            .map(|d| d.name.clone())
    }

    fn has(&self, id: &str) -> bool {
        self.devices.iter().any(|d| d.id == id)
    }

    /// Takes `last`'s queue when this push named the same active device
    /// without its player state.
    pub(super) fn keep_queue(&mut self, last: Option<Cluster>) {
        if self.queue.is_some() || self.active_id.is_empty() {
            return;
        }
        self.queue = last
            .filter(|c| c.active_id == self.active_id)
            .and_then(|c| c.queue);
    }
}

/// Connect's device types (`devices.proto`) as the picker's kinds.
pub(super) fn connect_kind(device_type: i32) -> &'static str {
    match device_type {
        1 | 14 => "computer",
        2 => "tablet",
        3 => "phone",
        4 | 6 | 8 | 11 | 103 => "speaker",
        5 | 7 | 10 => "tv",
        12 | 101 => "car",
        _ => "other",
    }
}

/// A pushed cluster's devices (offline ones left out), this one first.
pub(super) fn from_cluster(cluster: &ClusterProto, own_id: &str) -> Cluster {
    let mut devices: Vec<Device> = cluster
        .device
        .iter()
        .filter(|(_, info)| !info.is_offline)
        .map(|(id, info)| Device {
            id: id.clone(),
            name: info.name.clone(),
            kind: connect_kind(info.device_type.value()),
            active: *id == cluster.active_device_id,
            volume: Some(percent(info.volume.min(u16::MAX as u32) as u16)),
            is_this: id == own_id,
        })
        .collect();
    sort(&mut devices);
    let up_next = if cluster.active_device_id.is_empty() {
        Some(Vec::new())
    } else {
        cluster.player_state.as_ref().map(|p| {
            up_next(
                p.next_tracks
                    .iter()
                    .map(|t| (t.uri.as_str(), t.provider.as_str())),
            )
        })
    };
    let queue = if cluster.active_device_id.is_empty() {
        None
    } else {
        cluster.player_state.as_ref().map(Queue::from_player)
    };
    Cluster {
        active_id: cluster.active_device_id.clone(),
        devices,
        up_next,
        queue,
        remote: remote::from_cluster(cluster, own_id),
    }
}

/// This device first, then the active one, then by name.
fn sort(devices: &mut [Device]) {
    devices.sort_by(|a, b| {
        (!a.is_this, !a.active, a.name.to_lowercase()).cmp(&(
            !b.is_this,
            !b.active,
            b.name.to_lowercase(),
        ))
    });
}

/// Subscribes `session` to cluster pushes for run `id`. Called before the
/// Spirc starts the dealer; the task ends with the session's dealer.
pub(super) fn listen(app: &AppHandle, id: u64, session: &Session) {
    let updates = match session
        .dealer()
        .listen_for(CLUSTER_URI, Message::from_raw::<ClusterUpdate>)
    {
        Ok(updates) => updates,
        Err(e) => {
            elog!("[music] could not follow Spotify's device list: {e}");
            return;
        }
    };
    let app = app.clone();
    let session_id = session.session_id();
    let own_id = session.device_id().to_string();
    let account = session.username();
    tauri::async_runtime::spawn(async move {
        let mut updates = updates;
        let mut asked = None;
        while let Some(update) = updates.next().await {
            match update {
                Ok(update) => {
                    if let Some(cluster) = update.cluster.as_ref() {
                        let mut cluster = from_cluster(cluster, &own_id);
                        if let Some(remote) = cluster.remote.as_mut() {
                            remote::describe(&app, id, &account, remote, &mut asked);
                        }
                        state::set_cluster(&app, id, &session_id, cluster);
                    }
                }
                Err(e) => elog!("[music] unreadable Connect cluster update: {e}"),
            }
        }
    });
}

/// The devices of the last pushed cluster, or this device alone before the
/// first push.
pub(super) fn list(app: &AppHandle) -> Result<Vec<Device>, String> {
    let session = state::session(app).ok_or("Not connected to Spotify")?;
    let own_id = session.device_id().to_string();
    let devices = state::cluster(app).map(|c| c.devices).unwrap_or_default();
    Ok(with_this(devices, &own_id, &state::snapshot(app)))
}

/// Adds this device when the list lacks it.
pub(super) fn with_this(
    mut devices: Vec<Device>,
    own_id: &str,
    s: &state::Snapshot,
) -> Vec<Device> {
    if !devices.iter().any(|d| d.is_this) {
        devices.push(Device {
            id: own_id.to_string(),
            name: DEVICE_NAME.to_string(),
            kind: "computer",
            active: s.active,
            volume: Some(s.volume),
            is_this: true,
        });
        sort(&mut devices);
    }
    devices
}

/// Moves playback to `device_id` and plays.
pub(super) async fn transfer(app: &AppHandle, device_id: String) -> Result<(), String> {
    let session = state::session(app).ok_or("Not connected to Spotify")?;
    let own_id = session.device_id().to_string();
    if device_id == own_id {
        return control::play_here(app).await;
    }
    let cluster = state::cluster(app)
        .filter(|c| c.has(&device_id))
        .ok_or("That device is no longer in Spotify's device list")?;
    let from = if cluster.active_id.is_empty() {
        own_id
    } else {
        cluster.active_id
    };
    session
        .spclient()
        .transfer(&from, &device_id, None)
        .await
        .map(|_| ())
        .map_err(|e| format!("Spotify would not move playback ({e})"))
}

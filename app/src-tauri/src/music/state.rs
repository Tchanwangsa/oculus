//! What the frontend mirrors (`Snapshot`, sent whole as `music-state` on every
//! change) and the handles behind it: the live `Spirc` and `Session`, the
//! running session task, a pending login and a held duck. A session task tags
//! its writes with its run id, so a stopped one cannot change what the user
//! sees. The library's caches sit beside it (`library::Library`), under their
//! own lock.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use librespot_connect::Spirc;
use librespot_core::Session;
use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget, Manager};
use tokio::sync::watch;

use super::audio::Choice;
use super::devices::Cluster;
use super::duck::Duck;
use super::library::{Library, TrackRow};
use super::remote::{self, RemotePlayback};
use crate::shell::browser::MAIN;

pub const EVENT: &str = "music-state";
/// The name Spotify's device list shows.
pub const DEVICE_NAME: &str = "Oculus";
/// Full scale: librespot's softvol is a 60 dB log curve, so 50% is about
/// −30 dB. At 100% the Mac's own volume sets the loudness.
pub const INITIAL_VOLUME: u8 = 100;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Track {
    pub uri: String,
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    /// `https://i.scdn.co/image/<id>`; none when Spotify lists no cover.
    pub cover_url: Option<String>,
    pub duration_ms: u32,
}

/// A track to play after the current one.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NextTrack {
    pub uri: String,
    /// Added to the queue, as against next in the context.
    pub queued: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Snapshot {
    pub connected: bool,
    /// A login, a connect or a reconnect is under way.
    pub connecting: bool,
    /// No stored credentials, or Spotify refused them: `music_login` next.
    pub needs_login: bool,
    pub account: Option<String>,
    pub device_name: String,
    /// Oculus is Spotify's active Connect device; while false, Spotify drops
    /// every command but a transfer (`control.rs`).
    pub active: bool,
    /// The active device's name when it is another one, from the Connect
    /// cluster; none when unknown or when nothing is active.
    pub playing_on: Option<String>,
    /// The chosen output on this Mac; none is the system default.
    pub output: Option<String>,
    /// What the active device plays while it is another one, from the
    /// Connect cluster; the fields below stay Oculus's own.
    pub remote: Option<RemotePlayback>,
    pub track: Option<Track>,
    /// The position at `position_at_ms` (epoch ms). While `playing`, the UI
    /// adds the time since; Rust sends no per-second ticks.
    pub position_ms: u32,
    pub position_at_ms: u64,
    pub playing: bool,
    /// 0–100.
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: bool,
    /// What plays next, queued tracks first, every catalogue track Spotify
    /// lists (librespot keeps up to 80): from the Spirc's queue events while
    /// Oculus plays, from the Connect cluster's player state otherwise.
    pub up_next: Vec<NextTrack>,
    /// The playlist, album or other context the active device plays from,
    /// from the same two sources as `up_next`; none when unknown.
    pub context_uri: Option<String>,
    /// Oculus's previous goes back a track: the Spirc's queue event lists a
    /// previous track, or the context repeats.
    pub can_prev: bool,
    /// The last queue event listed a previous track.
    #[serde(skip)]
    pub(super) has_prev: bool,
    /// The context repeats (`repeat` also covers a repeated track).
    #[serde(skip)]
    pub(super) repeat_context: bool,
    /// The latest failure (connect, auth, keyd, a track that cannot play),
    /// kept until the next connect or load.
    pub error: Option<String>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            connected: false,
            connecting: false,
            needs_login: false,
            account: None,
            device_name: DEVICE_NAME.to_string(),
            active: false,
            playing_on: None,
            output: None,
            remote: None,
            track: None,
            position_ms: 0,
            position_at_ms: 0,
            playing: false,
            volume: INITIAL_VOLUME,
            shuffle: false,
            repeat: false,
            up_next: Vec::new(),
            context_uri: None,
            can_prev: false,
            has_prev: false,
            repeat_context: false,
            error: None,
        }
    }
}

impl Snapshot {
    pub(super) fn set_position(&mut self, position_ms: u32) {
        self.position_ms = position_ms;
        self.position_at_ms = now_ms();
    }

    /// Not connected and not trying, whatever came before.
    pub(super) fn set_idle(&mut self) {
        self.connected = false;
        self.connecting = false;
        self.set_unlinked();
    }

    /// The connection is gone: not active, nothing known about the others.
    pub(super) fn set_unlinked(&mut self) {
        self.playing = false;
        self.active = false;
        self.playing_on = None;
        self.remote = None;
        self.up_next.clear();
        self.context_uri = None;
        self.set_has_prev(false);
    }

    pub(super) fn set_has_prev(&mut self, has_prev: bool) {
        self.has_prev = has_prev;
        self.can_prev = has_prev || self.repeat_context;
    }

    pub(super) fn set_repeat(&mut self, context: bool, track: bool) {
        self.repeat = context || track;
        self.repeat_context = context;
        self.can_prev = self.has_prev || context;
    }
}

/// Whether previous tracks as the Spirc lists them hold one to go back to:
/// delimiters are not tracks.
pub(super) fn has_prev<'a>(uris: impl IntoIterator<Item = &'a str>) -> bool {
    uris.into_iter().any(|uri| uri != "spotify:delimiter")
}

/// Whether a next track's URI goes into `up_next`: catalogue tracks only,
/// so episodes, local files and delimiters are left out.
pub(super) fn listed(uri: &str) -> bool {
    uri.starts_with("spotify:track:")
}

/// A context URI as the snapshot keeps it: none when empty.
pub(super) fn context(uri: &str) -> Option<String> {
    Some(uri.to_string()).filter(|u| !u.is_empty())
}

/// Next tracks as (URI, provider) pairs, as the Spirc and the cluster list
/// them, into `up_next`, every one that is `listed`: Spotify's provider
/// `queue` marks a queued one.
pub(super) fn up_next<'a>(tracks: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<NextTrack> {
    tracks
        .into_iter()
        .filter(|(uri, _)| listed(uri))
        .map(|(uri, provider)| NextTrack {
            uri: uri.to_string(),
            queued: provider == "queue",
        })
        .collect()
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// librespot's volume (0–65535) as a percentage, rounded.
pub(super) fn percent(volume: u16) -> u8 {
    ((volume as u32 * 100 + u16::MAX as u32 / 2) / u16::MAX as u32) as u8
}

pub(super) fn from_percent(percent: u8) -> u16 {
    ((percent.min(100) as u32 * u16::MAX as u32 + 50) / 100) as u16
}

struct Run {
    id: u64,
    stop: watch::Sender<bool>,
}

#[derive(Default)]
struct Inner {
    snapshot: Snapshot,
    /// The running task's device handle, once it has connected.
    spirc: Option<Spirc>,
    /// That connection's session, for the library's and transfer's requests.
    session: Option<Session>,
    duck: Duck,
    /// The Connect cluster from the last dealer push, for this connection.
    cluster: Option<Cluster>,
    run: Option<Run>,
    next_run: u64,
    /// Set to cancel the pending login's callback listener.
    login: Option<Arc<AtomicBool>>,
}

pub struct MusicState {
    inner: Mutex<Inner>,
    library: Library,
    /// The output choice, shared with the player's sink.
    audio: Arc<Choice>,
    /// The snapshot's `active`, for a command waiting on a transfer.
    active: watch::Sender<bool>,
}

impl Default for MusicState {
    fn default() -> Self {
        Self {
            inner: Mutex::default(),
            library: Library::default(),
            audio: Arc::default(),
            active: watch::Sender::new(false),
        }
    }
}

fn with<T>(app: &AppHandle, f: impl FnOnce(&mut Inner) -> T) -> T {
    let state = app.state::<MusicState>();
    let mut inner = state.inner.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut inner)
}

/// The library's caches and rate-limit gate.
pub(super) fn library(app: &AppHandle) -> &Library {
    &app.state::<MusicState>().inner().library
}

/// Applies `f` to the snapshot under the lock and emits it, outside the
/// lock, when it changed.
fn change(app: &AppHandle, f: impl FnOnce(&mut Inner) -> bool) {
    let changed = with(app, |inner| {
        let before = inner.snapshot.clone();
        if !f(inner) || inner.snapshot == before {
            return None;
        }
        Some(inner.snapshot.clone())
    });
    if let Some(snapshot) = changed {
        app.state::<MusicState>().active.send_if_modified(|active| {
            std::mem::replace(active, snapshot.active) != snapshot.active
        });
        app.emit_to(EventTarget::webview(MAIN), EVENT, snapshot)
            .ok();
    }
}

pub(super) fn snapshot(app: &AppHandle) -> Snapshot {
    with(app, |inner| inner.snapshot.clone())
}

pub(super) fn update(app: &AppHandle, f: impl FnOnce(&mut Snapshot)) {
    change(app, |inner| {
        f(&mut inner.snapshot);
        true
    });
}

/// `update`, only while run `id` is the current one.
pub(super) fn update_run(app: &AppHandle, id: u64, f: impl FnOnce(&mut Snapshot)) {
    change(app, |inner| {
        if inner.run.as_ref().map(|r| r.id) != Some(id) {
            return false;
        }
        f(&mut inner.snapshot);
        true
    });
}

/// Registers a new session task, or `None` when one is already running.
pub(super) fn begin_run(app: &AppHandle) -> Option<(u64, watch::Receiver<bool>)> {
    let mut started = None;
    change(app, |inner| {
        if inner.run.is_some() {
            return false;
        }
        inner.next_run += 1;
        let (stop, rx) = watch::channel(false);
        inner.run = Some(Run {
            id: inner.next_run,
            stop,
        });
        started = Some((inner.next_run, rx));
        let s = &mut inner.snapshot;
        s.connecting = true;
        s.needs_login = false;
        s.error = None;
        true
    });
    started
}

/// The task's own end: unregisters run `id` and applies `f`.
pub(super) fn end_run(app: &AppHandle, id: u64, f: impl FnOnce(&mut Snapshot)) {
    change(app, |inner| {
        if inner.run.as_ref().map(|r| r.id) != Some(id) {
            return false;
        }
        inner.run = None;
        inner.drop_link();
        f(&mut inner.snapshot);
        true
    });
}

impl Inner {
    /// Forgets the connection: its device handle, its session and any duck.
    fn drop_link(&mut self) -> Option<Spirc> {
        self.session = None;
        self.cluster = None;
        self.duck = Duck::default();
        self.spirc.take()
    }
}

/// Hands run `id`'s device handle and session over for commands (`None` when
/// its connection dropped). A stopped run gets its handle back to shut down.
pub(super) fn set_link(
    app: &AppHandle,
    id: u64,
    link: Option<(Spirc, Session)>,
) -> Result<(), Spirc> {
    with(app, |inner| {
        if inner.run.as_ref().map(|r| r.id) != Some(id) {
            return link.map_or(Ok(()), |(spirc, _)| Err(spirc));
        }
        inner.drop_link();
        if let Some((spirc, session)) = link {
            inner.spirc = Some(spirc);
            inner.session = Some(session);
        }
        Ok(())
    })
}

/// The connected session.
pub(super) fn session(app: &AppHandle) -> Option<Session> {
    with(app, |inner| inner.session.clone())
}

/// Stops the running session task, if any: the device leaves Spotify's list
/// and the task shuts its session down on its own. Its later writes are
/// ignored. True when one was running.
pub(super) fn stop_run(app: &AppHandle) -> bool {
    let mut stopped = false;
    change(app, |inner| {
        let Some(run) = inner.run.take() else {
            return false;
        };
        if let Some(spirc) = inner.drop_link() {
            spirc.shutdown().ok();
        }
        run.stop.send_replace(true);
        stopped = true;
        inner.snapshot.set_idle();
        true
    });
    stopped
}

/// Runs a playback command on the connected device.
pub(super) fn with_spirc(
    app: &AppHandle,
    f: impl FnOnce(&Spirc) -> Result<(), librespot_core::Error>,
) -> Result<(), String> {
    with(app, |inner| match inner.spirc.as_ref() {
        Some(spirc) => f(spirc).map_err(|e| e.to_string()),
        None => Err("Not connected to Spotify".into()),
    })
}

/// Whether this device is active, or `None` without a connected device.
pub(super) fn active(app: &AppHandle) -> Option<bool> {
    with(app, |inner| {
        inner.spirc.as_ref().map(|_| inner.snapshot.active)
    })
}

/// Follows the snapshot's `active`.
pub(super) fn watch_active(app: &AppHandle) -> watch::Receiver<bool> {
    app.state::<MusicState>().active.subscribe()
}

/// The output choice the player's sink reads.
pub(super) fn audio(app: &AppHandle) -> Arc<Choice> {
    app.state::<MusicState>().audio.clone()
}

/// Chooses the output (`None`: the system default); the sink moves to it on
/// its next write.
pub(super) fn set_output(app: &AppHandle, name: Option<String>) {
    let choice = audio(app);
    choice.set(name);
    let chosen = choice.get().0;
    update(app, |s| s.output = chosen);
}

/// Takes back an error only while it is still the one shown.
pub(super) fn clear_error_if(app: &AppHandle, id: u64, message: &str) {
    update_run(app, id, |s| {
        if s.error.as_deref() == Some(message) {
            s.error = None;
        }
    });
}

/// The Connect cluster from the last push, while connected.
pub(super) fn cluster(app: &AppHandle) -> Option<Cluster> {
    with(app, |inner| inner.cluster.clone())
}

/// A cluster push heard on the session `session_id` of run `id`; a push
/// from any other session is dropped.
pub(super) fn set_cluster(app: &AppHandle, id: u64, session_id: &str, mut cluster: Cluster) {
    change(app, |inner| {
        let current = inner
            .session
            .as_ref()
            .is_some_and(|s| s.session_id() == session_id);
        if inner.run.as_ref().map(|r| r.id) != Some(id) || !current {
            return false;
        }
        cluster.keep_queue(inner.cluster.take());
        apply_cluster(&mut inner.snapshot, &mut cluster);
        inner.cluster = Some(cluster);
        true
    });
}

/// A pushed cluster's effect on the snapshot. Another device active with no
/// player state in the push keeps what was known of that device.
pub(super) fn apply_cluster(s: &mut Snapshot, cluster: &mut Cluster) {
    s.playing_on = cluster.playing_on();
    if let Some(next) = &cluster.up_next {
        s.up_next = next.clone();
        s.context_uri = cluster.queue.as_ref().and_then(|q| context(&q.context_uri));
    }
    if cluster.active_id.is_empty() {
        s.set_has_prev(false);
    }
    s.remote = match (cluster.remote.take(), &s.playing_on) {
        (Some(remote), _) => Some(remote),
        (None, Some(name)) => s.remote.take().filter(|r| &r.device_name == name),
        (None, None) => None,
    };
}

/// A track row for what the device playing elsewhere plays, fetched after
/// the push that named it; dropped when that device has moved on.
pub(super) fn set_remote_row(app: &AppHandle, id: u64, row: &TrackRow) {
    change(app, |inner| {
        if inner.run.as_ref().map(|r| r.id) != Some(id) {
            return false;
        }
        match inner.snapshot.remote.as_mut() {
            Some(playing) if playing.uri == row.uri => {
                remote::with_row(playing, row);
                true
            }
            _ => false,
        }
    });
}

/// Sets the volume the user chose; a held duck is dropped, since their
/// choice wins.
pub(super) fn set_volume(app: &AppHandle, percent: u8) -> Result<(), String> {
    with_duck(app, |duck, _, spirc| {
        duck.forget();
        spirc.set_volume(from_percent(percent))
    })
}

/// `Some(level)` lowers the volume to `level`% of the volume before the first
/// duck; `None` restores that volume, or does nothing without a held duck.
pub(super) fn duck(app: &AppHandle, level: Option<u8>) -> Result<(), String> {
    with_duck(app, |duck, volume, spirc| {
        let target = match level {
            Some(level) => Some(duck.duck(volume, level)),
            None => duck.unduck(),
        };
        match target {
            Some(target) => spirc.set_volume(from_percent(target)),
            None => Ok(()),
        }
    })
}

/// A volume the player reported, which may drop a held duck.
pub(super) fn volume_seen(app: &AppHandle, percent: u8) {
    with(app, |inner| inner.duck.seen(percent));
}

/// Runs `f` with the duck, the current volume and the connected device.
fn with_duck(
    app: &AppHandle,
    f: impl FnOnce(&mut Duck, u8, &Spirc) -> Result<(), librespot_core::Error>,
) -> Result<(), String> {
    with(app, |inner| {
        let Inner {
            spirc,
            duck,
            snapshot,
            ..
        } = inner;
        match spirc.as_ref() {
            Some(spirc) => f(duck, snapshot.volume, spirc).map_err(|e| e.to_string()),
            None => Err("Not connected to Spotify".into()),
        }
    })
}

/// A new login's cancel flag; a pending one is cancelled first.
pub(super) fn begin_login(app: &AppHandle) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let old = with(app, |inner| inner.login.replace(flag.clone()));
    if let Some(old) = old {
        old.store(true, Ordering::Relaxed);
    }
    flag
}

pub(super) fn cancel_login(app: &AppHandle) {
    if let Some(flag) = with(app, |inner| inner.login.take()) {
        flag.store(true, Ordering::Relaxed);
    }
}

/// True when `flag` is still the current login (and clears it): a login that
/// was cancelled or replaced must not touch the snapshot.
pub(super) fn end_login(app: &AppHandle, flag: &Arc<AtomicBool>) -> bool {
    with(app, |inner| {
        let current = inner.login.as_ref().is_some_and(|f| Arc::ptr_eq(f, flag));
        if current {
            inner.login = None;
        }
        current
    })
}

/// On app exit: the device leaves Spotify's list rather than lingering.
pub fn shutdown(app: &AppHandle) {
    cancel_login(app);
    stop_run(app);
}

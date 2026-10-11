//! What the frontend mirrors (`Snapshot`, sent whole as `music-state` on every
//! change) and the handles behind it: the live `Spirc`, the running session
//! task and a pending login. A session task tags its writes with its run id,
//! so a stopped one cannot change what the user sees.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use librespot_connect::Spirc;
use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget, Manager};
use tokio::sync::watch;

use crate::shell::browser::MAIN;

pub const EVENT: &str = "music-state";
/// The name Spotify's device list shows.
pub const DEVICE_NAME: &str = "Oculus";
pub const INITIAL_VOLUME: u8 = 50;

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

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Snapshot {
    pub connected: bool,
    /// A login, a connect or a reconnect is under way.
    pub connecting: bool,
    /// No stored credentials, or Spotify refused them: `music_login` next.
    pub needs_login: bool,
    pub account: Option<String>,
    pub device_name: String,
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
            track: None,
            position_ms: 0,
            position_at_ms: 0,
            playing: false,
            volume: INITIAL_VOLUME,
            shuffle: false,
            repeat: false,
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
        self.playing = false;
    }
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
    run: Option<Run>,
    next_run: u64,
    /// Set to cancel the pending login's callback listener.
    login: Option<Arc<AtomicBool>>,
}

#[derive(Default)]
pub struct MusicState(Mutex<Inner>);

fn with<T>(app: &AppHandle, f: impl FnOnce(&mut Inner) -> T) -> T {
    let state = app.state::<MusicState>();
    let mut inner = state.0.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut inner)
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
        inner.spirc = None;
        f(&mut inner.snapshot);
        true
    });
}

/// Hands run `id`'s device handle over for commands (`None` when its
/// connection dropped). A stopped run gets its handle back to shut down.
pub(super) fn set_spirc(app: &AppHandle, id: u64, spirc: Option<Spirc>) -> Result<(), Spirc> {
    with(app, |inner| {
        if inner.run.as_ref().map(|r| r.id) == Some(id) {
            inner.spirc = spirc;
            Ok(())
        } else {
            spirc.map_or(Ok(()), Err)
        }
    })
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
        if let Some(spirc) = inner.spirc.take() {
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

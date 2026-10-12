//! The session task: one player, and a Session + Spirc pair rebuilt whenever
//! the connection drops without a stop. A Session cannot be reused once its
//! Spirc has ended (its dealer listeners stay registered), so each attempt
//! gets a new one and the player is handed it. A drop, or the Mac waking, is
//! noticed within a second (`link.rs`) and reconnects at once; attempts that fail back off
//! (`policy.rs`). What Oculus was playing at the drop is resumed once the new
//! connection is up (`resume.rs`). An auth error stops the task with the
//! reason in the snapshot: it is never retried in a loop.

mod link;
pub(super) mod policy;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use librespot_connect::{ConnectConfig, Spirc};
use librespot_core::authentication::Credentials;
use librespot_core::config::DeviceType;
use librespot_core::error::ErrorKind;
use librespot_core::{Error, Session, SessionConfig};
use librespot_playback::config::PlayerConfig;
use librespot_playback::mixer::{self, Mixer, MixerConfig};
use librespot_playback::player::Player;
use tauri::AppHandle;
use tokio::sync::{watch, Notify};

use super::audio::{Note, Switching};
use super::credentials::{self, Loaded, Saved};
use super::state::{self, from_percent, Begun, DEVICE_NAME};
use super::{devices, events, logs, netwatch, resume};
use crate::runtime::blocking;
use link::Unlinked;
use policy::{Backoff, Stamp};

/// How long a stopped Spirc gets to pause playback and leave the device list.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);
/// How long a dropped link's Spirc gets to clear its device state and close
/// its dealer. Its requests may hang on a dead network, and the next attempt
/// must not start before they land or they would clear the new link's state.
const DROP_GRACE: Duration = Duration::from_secs(3);

pub(super) enum Start {
    /// From the credentials keyd holds.
    Stored,
    /// From a login's access token, as a new device.
    Token(String),
}

/// How a task ended, which decides the snapshot it leaves.
enum End {
    /// `stop_run` asked; it has already set the snapshot.
    Stopped,
    /// No usable credentials, or Spotify refused them.
    NeedsLogin(Option<String>),
    Failed(String),
}

/// Starts the session task, unless one is running already (false).
pub(super) fn start(app: &AppHandle, how: Start) -> bool {
    let Some(begun) = state::begin_run(app) else {
        return false;
    };
    logs::install(app);
    netwatch::start(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let id = begun.id;
        match run(&app, begun, how).await {
            End::Stopped => {}
            End::NeedsLogin(why) => state::end_run(&app, id, |s| {
                s.set_idle();
                s.needs_login = true;
                s.error = why;
            }),
            End::Failed(why) => state::end_run(&app, id, |s| {
                s.set_idle();
                s.error = Some(why);
            }),
        }
    });
    true
}

fn is_auth_error(e: &Error) -> bool {
    matches!(
        e.kind,
        ErrorKind::PermissionDenied | ErrorKind::Unauthenticated
    )
}

/// librespot takes the volume from here alone, on every connect (there is no
/// `Cache` to restore one from), so a reconnect keeps the snapshot's volume:
/// `INITIAL_VOLUME` until the user changes it.
fn connect_config(volume: u8) -> ConnectConfig {
    ConnectConfig {
        name: DEVICE_NAME.to_string(),
        device_type: DeviceType::Computer,
        initial_volume: from_percent(volume),
        // What plays next, for the snapshot's `up_next`.
        emit_set_queue_events: true,
        ..ConnectConfig::default()
    }
}

async fn run(app: &AppHandle, begun: Begun, how: Start) -> End {
    let Begun { id, stop, wake } = begun;
    let (credentials, saved) = match how {
        Start::Stored => match blocking::run(credentials::load).await {
            Ok(Loaded::Saved(saved)) => (saved.credentials.clone(), Some(saved)),
            Ok(Loaded::Nothing) => return End::NeedsLogin(None),
            Ok(Loaded::Unreadable(why)) => return End::NeedsLogin(Some(why)),
            Err(why) => return End::Failed(why),
        },
        Start::Token(token) => (Credentials::with_access_token(token), None),
    };
    // The stored account is known before Spotify answers: what it last
    // played shows while connecting.
    if let Some(account) = saved.as_ref().and_then(|s| s.credentials.username.clone()) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { resume::recall(&app, id, &account).await });
    }
    let config = SessionConfig {
        device_id: saved
            .as_ref()
            .map(|s| s.device_id.clone())
            .unwrap_or_else(|| SessionConfig::default().device_id),
        ..SessionConfig::default()
    };

    let mixer = match mixer::find(None).map(|open| open(MixerConfig::default())) {
        Some(Ok(mixer)) => mixer,
        Some(Err(e)) => return End::Failed(format!("No audio mixer for Spotify: {e}")),
        None => return End::Failed("librespot was built without a mixer".into()),
    };
    let choice = state::audio(app);
    let report_app = app.clone();
    let report: super::audio::Report = std::sync::Arc::new(move |note| match note {
        Note::Error(message) => state::update_run(&report_app, id, |s| s.error = Some(message)),
        Note::Cleared(message) => state::clear_error_if(&report_app, id, &message),
    });
    let session = Session::new(config.clone(), None);
    let player = Player::new(
        PlayerConfig {
            normalisation: false,
            ..PlayerConfig::default()
        },
        session.clone(),
        mixer.get_soft_volume(),
        move || Box::new(Switching::new(choice, report)),
    );
    tauri::async_runtime::spawn(events::run(
        app.clone(),
        id,
        player.get_player_event_channel(),
    ));

    let link = Link {
        app,
        id,
        config,
        player: player.clone(),
        mixer,
        stop,
        wake,
    };
    let end = link.connect(session, credentials, saved).await;
    // Player's Drop joins its thread.
    tokio::task::spawn_blocking(move || drop(player));
    end
}

struct Link<'a> {
    app: &'a AppHandle,
    id: u64,
    config: SessionConfig,
    player: Arc<Player>,
    mixer: Arc<dyn Mixer>,
    stop: watch::Receiver<bool>,
    /// `music_wake`'s nudge: check the link now, or cut a backoff short.
    wake: Arc<Notify>,
}

impl Link<'_> {
    async fn connect(
        mut self,
        mut session: Session,
        mut credentials: Credentials,
        mut saved: Option<Saved>,
    ) -> End {
        let (app, id) = (self.app, self.id);
        let mut fresh = true;
        let mut backoff = Backoff::default();
        loop {
            if *self.stop.borrow() {
                return End::Stopped;
            }
            if !fresh {
                if !session.is_invalid() {
                    session.shutdown();
                }
                session = Session::new(self.config.clone(), None);
                self.player.set_session(session.clone());
            }
            fresh = false;

            devices::listen(app, id, &session);
            let attempt = Spirc::new(
                connect_config(state::snapshot(app).volume),
                session.clone(),
                credentials.clone(),
                self.player.clone(),
                self.mixer.clone(),
            );
            let result = tokio::select! {
                r = attempt => r,
                _ = self.stop.changed() => {
                    session.shutdown();
                    return End::Stopped;
                }
            };

            let wait = match result {
                Ok((spirc, task)) => {
                    let announced = spirc.announced();
                    devices::follow_announced(app, id, &session, announced.clone());
                    let username = session.username();
                    let reusable = Saved::stored(
                        username.clone(),
                        session.auth_data(),
                        self.config.device_id.clone(),
                    );
                    credentials = reusable.credentials.clone();
                    if let Err(spirc) = state::set_link(app, id, Some((spirc, session.clone()))) {
                        spirc.shutdown().ok();
                    }
                    state::update_run(app, id, |s| {
                        s.connected = true;
                        s.connecting = false;
                        s.reconnecting = false;
                        s.needs_login = false;
                        s.account = Some(username.clone());
                        s.error = None;
                    });
                    if saved.as_ref() != Some(&reusable) {
                        keep(app, id, reusable.clone());
                        saved = Some(reusable);
                    }
                    tauri::async_runtime::spawn(resume::after_connect(
                        app.clone(),
                        id,
                        username,
                        session.session_id(),
                        announced,
                    ));

                    let connected = Stamp::now();
                    tokio::pin!(task);
                    let why = self.linked(&session, task.as_mut()).await;
                    if why == Unlinked::Stopped {
                        // `stop_run` has sent the Spirc its shutdown.
                        if tokio::time::timeout(SHUTDOWN_GRACE, &mut task)
                            .await
                            .is_err()
                        {
                            elog!("[music] the Spotify device did not shut down within 10 s");
                        }
                        session.shutdown();
                        return End::Stopped;
                    }
                    let lived = connected.lived();
                    let wait = backoff.after_drop(lived);
                    elog!(
                        "[music] the Spotify connection dropped after {} s ({}); reconnecting {}",
                        lived.as_secs(),
                        why.reason(),
                        when(wait)
                    );
                    self.unlink(&session, task, why).await;
                    wait
                }
                Err(e) if is_auth_error(&e) => {
                    elog!("[music] Spotify refused the login: {e}");
                    session.shutdown();
                    return End::NeedsLogin(Some(format!(
                        "Spotify refused the login ({e}). Log in again."
                    )));
                }
                Err(e) => {
                    let wait = backoff.after_failure();
                    elog!(
                        "[music] connecting to Spotify failed: {e}; retrying {}",
                        when(wait)
                    );
                    state::update_run(app, id, |s| {
                        s.connecting = true;
                        s.error = Some(format!(
                            "Could not connect to Spotify ({e}); retrying in {} s",
                            wait.as_secs()
                        ));
                    });
                    wait
                }
            };

            if !wait.is_zero() {
                tokio::select! {
                    _ = tokio::time::sleep(wait) => {}
                    _ = self.wake.notified() => {}
                    _ = self.stop.changed() => return End::Stopped,
                }
            }
        }
    }
}

/// When the next attempt comes, for the log.
fn when(wait: Duration) -> String {
    if wait.is_zero() {
        "now".into()
    } else {
        format!("in {} s", wait.as_secs())
    }
}

/// Saves the reusable credentials in keyd off the runtime, so the Spirc task
/// is polled meanwhile (keyd may be waiting on a keychain prompt). Playback
/// goes on if it fails; the snapshot says the login was not kept.
fn keep(app: &AppHandle, id: u64, saved: Saved) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(why) = blocking::run(move || credentials::save(&saved)).await {
            elog!("[music] could not keep the Spotify login: {why}");
            state::update_run(&app, id, |s| {
                s.error = Some(format!("Signed in, but the login was not kept: {why}"));
            });
        }
    });
}

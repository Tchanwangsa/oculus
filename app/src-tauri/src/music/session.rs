//! The session task: one player, and a Session + Spirc pair rebuilt whenever
//! the connection drops without a stop. A Session cannot be reused once its
//! Spirc has ended (its dealer listeners stay registered), so each attempt
//! gets a new one and the player is handed it. Backoff is 2 s doubling to
//! 60 s, reset once a connection has lived a minute. An auth error stops the
//! task with the reason in the snapshot: it is never retried in a loop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use librespot_connect::{ConnectConfig, Spirc};
use librespot_core::authentication::Credentials;
use librespot_core::config::DeviceType;
use librespot_core::error::ErrorKind;
use librespot_core::{Error, Session, SessionConfig};
use librespot_playback::audio_backend;
use librespot_playback::config::{AudioFormat, PlayerConfig};
use librespot_playback::mixer::{self, Mixer, MixerConfig};
use librespot_playback::player::Player;
use tauri::AppHandle;
use tokio::sync::watch;

use super::credentials::{self, Loaded, Saved};
use super::state::{self, from_percent, DEVICE_NAME, INITIAL_VOLUME};
use super::{events, logs};
use crate::runtime::blocking;

pub(super) const BACKOFF_MIN: Duration = Duration::from_secs(2);
pub(super) const BACKOFF_MAX: Duration = Duration::from_secs(60);
const STABLE_AFTER: Duration = Duration::from_secs(60);
/// How long a stopped Spirc gets to pause playback and leave the device list.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

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
    let Some((id, stop)) = state::begin_run(app) else {
        return false;
    };
    logs::install(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match run(&app, id, how, stop).await {
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

pub(super) fn next_backoff(backoff: Duration) -> Duration {
    (backoff * 2).min(BACKOFF_MAX)
}

fn is_auth_error(e: &Error) -> bool {
    matches!(
        e.kind,
        ErrorKind::PermissionDenied | ErrorKind::Unauthenticated
    )
}

fn connect_config() -> ConnectConfig {
    ConnectConfig {
        name: DEVICE_NAME.to_string(),
        device_type: DeviceType::Computer,
        initial_volume: from_percent(INITIAL_VOLUME),
        ..ConnectConfig::default()
    }
}

async fn run(app: &AppHandle, id: u64, how: Start, stop: watch::Receiver<bool>) -> End {
    let (credentials, saved) = match how {
        Start::Stored => match blocking::run(credentials::load).await {
            Ok(Loaded::Saved(saved)) => (saved.credentials.clone(), Some(saved)),
            Ok(Loaded::Nothing) => return End::NeedsLogin(None),
            Ok(Loaded::Unreadable(why)) => return End::NeedsLogin(Some(why)),
            Err(why) => return End::Failed(why),
        },
        Start::Token(token) => (Credentials::with_access_token(token), None),
    };
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
    let Some(sink) = audio_backend::find(None) else {
        return End::Failed("librespot was built without an audio backend".into());
    };
    let session = Session::new(config.clone(), None);
    let player = Player::new(
        PlayerConfig {
            normalisation: false,
            ..PlayerConfig::default()
        },
        session.clone(),
        mixer.get_soft_volume(),
        move || sink(None, AudioFormat::default()),
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
        let mut backoff = BACKOFF_MIN;
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

            let attempt = Spirc::new(
                connect_config(),
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

            match result {
                Ok((spirc, task)) => {
                    let username = session.username();
                    let reusable = Saved::stored(
                        username.clone(),
                        session.auth_data(),
                        self.config.device_id.clone(),
                    );
                    credentials = reusable.credentials.clone();
                    if let Err(spirc) = state::set_spirc(app, id, Some(spirc)) {
                        spirc.shutdown().ok();
                    }
                    state::update_run(app, id, |s| {
                        s.connected = true;
                        s.connecting = false;
                        s.needs_login = false;
                        s.account = Some(username);
                        s.error = None;
                    });
                    if saved.as_ref() != Some(&reusable) {
                        keep(app, id, reusable.clone());
                        saved = Some(reusable);
                    }

                    let connected_at = Instant::now();
                    tokio::pin!(task);
                    let stopping = tokio::select! {
                        _ = &mut task => false,
                        _ = self.stop.changed() => true,
                    };
                    if stopping {
                        // `stop_run` has sent the Spirc its shutdown.
                        if tokio::time::timeout(SHUTDOWN_GRACE, &mut task)
                            .await
                            .is_err()
                        {
                            eprintln!("[music] the Spotify device did not shut down within 10 s");
                        }
                        session.shutdown();
                        return End::Stopped;
                    }

                    state::set_spirc(app, id, None).ok();
                    if connected_at.elapsed() >= STABLE_AFTER {
                        backoff = BACKOFF_MIN;
                    }
                    eprintln!(
                        "[music] the Spotify connection dropped after {} s; reconnecting in {} s",
                        connected_at.elapsed().as_secs(),
                        backoff.as_secs()
                    );
                    state::update_run(app, id, |s| {
                        s.connected = false;
                        s.connecting = true;
                        s.playing = false;
                        s.error = Some(format!(
                            "Lost the connection to Spotify; reconnecting in {} s",
                            backoff.as_secs()
                        ));
                    });
                }
                Err(e) if is_auth_error(&e) => {
                    eprintln!("[music] Spotify refused the login: {e}");
                    session.shutdown();
                    return End::NeedsLogin(Some(format!(
                        "Spotify refused the login ({e}). Log in again."
                    )));
                }
                Err(e) => {
                    eprintln!(
                        "[music] connecting to Spotify failed: {e}; retrying in {} s",
                        backoff.as_secs()
                    );
                    state::update_run(app, id, |s| {
                        s.connecting = true;
                        s.error = Some(format!(
                            "Could not connect to Spotify ({e}); retrying in {} s",
                            backoff.as_secs()
                        ));
                    });
                }
            }

            tokio::select! {
                _ = tokio::time::sleep(backoff) => {}
                _ = self.stop.changed() => return End::Stopped,
            }
            backoff = next_backoff(backoff);
        }
    }
}

/// Saves the reusable credentials in keyd off the runtime, so the Spirc task
/// is polled meanwhile (keyd may be waiting on a keychain prompt). Playback
/// goes on if it fails; the snapshot says the login was not kept.
fn keep(app: &AppHandle, id: u64, saved: Saved) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(why) = blocking::run(move || credentials::save(&saved)).await {
            eprintln!("[music] could not keep the Spotify login: {why}");
            state::update_run(&app, id, |s| {
                s.error = Some(format!("Signed in, but the login was not kept: {why}"));
            });
        }
    });
}

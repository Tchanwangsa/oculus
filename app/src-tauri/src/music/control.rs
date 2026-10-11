//! Where a playback command goes. librespot's Spirc drops every command but
//! `activate` and `transfer` while it is not the active Connect device, so a
//! command finds where playback is (`At`) and what it needs (`Need`), and
//! `step` picks one of:
//!
//! - **Send** here, while Oculus is the active device.
//! - **Remote**: another device is active (the last cluster push), so the
//!   command goes to it over Spotify's connect-state API (`remote/`).
//! - **Pull** (nothing active anywhere, or "Play here"): `transfer`, which
//!   asks Spotify to move the current playback here. It lands later as a
//!   dealer request, so the command waits (bounded) for the player's
//!   `SessionConnected`, which sets the snapshot's `active`. If it never
//!   lands, `activate` takes over locally instead.
//! - **Activate** (a load with nothing active): `activate` alone. The Spirc
//!   handles its commands in order and `activate` sets it active at once, so
//!   the load that follows is not dropped.
//! - **Skip**: nothing at all.
//! - **Refuse** (add to queue with nothing active): there is no queue to
//!   add to.

use std::time::Duration;

use tauri::AppHandle;

use super::remote::{self, RemoteCommand};
use super::state;

/// How long a pull waits for Spotify's transfer to reach this device.
const PULL_WAIT: Duration = Duration::from_secs(5);

/// Why add to queue was refused.
pub(super) const QUEUE_REFUSAL: &str = "Play something first, then add to the queue";

/// What a command needs while Oculus is not the active device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Need {
    /// Playback here, wherever it is now: "Play here".
    Pull,
    /// The device playing: another one's by remote, else pulled here.
    Control,
    /// Like `Control`, but with nothing active a pause pulls nothing.
    NeverPull,
    /// A load: another device's by remote, else activated here.
    Load,
    /// The active device's queue; refused with nothing active.
    Queue,
    /// This device only: what the exclusive rule and a duck send, which must
    /// never touch another device.
    IfActive,
}

/// Where playback is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum At {
    Here,
    Elsewhere,
    Nowhere,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    Send,
    Remote,
    PullThenSend,
    ActivateThenSend,
    Skip,
    Refuse,
}

pub(super) fn step(at: At, need: Need) -> Step {
    match (at, need) {
        (At::Here, _) => Step::Send,
        (_, Need::IfActive) => Step::Skip,
        (_, Need::Pull) | (At::Nowhere, Need::Control) => Step::PullThenSend,
        (At::Elsewhere, Need::Control | Need::NeverPull | Need::Load | Need::Queue) => Step::Remote,
        (At::Nowhere, Need::NeverPull) => Step::Skip,
        (At::Nowhere, Need::Load) => Step::ActivateThenSend,
        (At::Nowhere, Need::Queue) => Step::Refuse,
    }
}

/// Where a command goes once `route` has prepared it.
pub(super) enum Route {
    /// Oculus is active: send through the Spirc.
    Here,
    There(remote::Device),
    Skip,
}

/// Prepares a command for where playback is (see the module docs): pulls or
/// activates as `need` says, or names the device to send it to.
pub(super) async fn route(app: &AppHandle, need: Need) -> Result<Route, String> {
    let active = state::active(app).ok_or("Not connected to Spotify")?;
    let elsewhere = if active { None } else { other_device(app) };
    let at = match (active, &elsewhere) {
        (true, _) => At::Here,
        (false, Some(_)) => At::Elsewhere,
        (false, None) => At::Nowhere,
    };
    match step(at, need) {
        Step::Send => Ok(Route::Here),
        Step::Skip => Ok(Route::Skip),
        Step::Refuse => Err(QUEUE_REFUSAL.into()),
        Step::Remote => elsewhere
            .map(Route::There)
            .ok_or_else(|| "No device to send to".into()),
        Step::ActivateThenSend => {
            state::with_spirc(app, |spirc| spirc.activate())?;
            Ok(Route::Here)
        }
        Step::PullThenSend => {
            pull(app).await?;
            Ok(Route::Here)
        }
    }
}

/// Runs `local` here or sends `remote` to the device playing elsewhere, as
/// `route` decides.
pub(super) async fn send(
    app: &AppHandle,
    need: Need,
    local: impl FnOnce() -> Result<(), String>,
    remote: impl FnOnce(&remote::Device) -> RemoteCommand,
) -> Result<(), String> {
    match route(app, need).await? {
        Route::Here => local(),
        Route::There(device) => {
            let command = remote(&device);
            remote::send(app, &device, command).await
        }
        Route::Skip => Ok(()),
    }
}

/// Runs `local` while Oculus is the active device; nothing otherwise.
pub(super) async fn if_active(
    app: &AppHandle,
    local: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match route(app, Need::IfActive).await? {
        Route::Here => local(),
        Route::There(_) | Route::Skip => Ok(()),
    }
}

/// Moves playback here and plays.
pub(super) async fn play_here(app: &AppHandle) -> Result<(), String> {
    match route(app, Need::Pull).await? {
        Route::Here => state::with_spirc(app, |spirc| spirc.play()),
        Route::There(_) | Route::Skip => Ok(()),
    }
}

/// The active device from the last cluster push, when it is another one.
fn other_device(app: &AppHandle) -> Option<remote::Device> {
    let own_id = state::session(app)?.device_id().to_string();
    let cluster = state::cluster(app)?;
    if cluster.active_id.is_empty() || cluster.active_id == own_id {
        return None;
    }
    let snapshot = state::snapshot(app);
    let name = cluster
        .devices
        .iter()
        .find(|d| d.id == cluster.active_id)
        .map(|d| d.name.clone())
        .or_else(|| snapshot.remote.as_ref().map(|r| r.device_name.clone()))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "the other device".into());
    Some(remote::Device {
        from: own_id,
        to: cluster.active_id,
        name,
        playing: snapshot.remote.as_ref().is_some_and(|r| r.playing),
        can_prev: snapshot.remote.as_ref().is_none_or(|r| r.can_prev),
    })
}

async fn pull(app: &AppHandle) -> Result<(), String> {
    let mut active = state::watch_active(app);
    state::with_spirc(app, |spirc| spirc.transfer(None))?;
    let landed = tokio::time::timeout(PULL_WAIT, active.wait_for(|on| *on))
        .await
        .is_ok_and(|r| r.is_ok());
    if !landed {
        elog!("[music] Spotify's transfer did not arrive within 5 s; taking over locally");
        state::with_spirc(app, |spirc| spirc.activate())?;
    }
    Ok(())
}

//! Playback commands against a device that may not be the active one.
//! librespot's Spirc drops every command but `activate` and `transfer` while
//! another device holds playback, so a command first makes Oculus active:
//!
//! - **Pull** (play, next, seek, volume…): `transfer`, which asks Spotify to
//!   move the current playback here. It lands later as a dealer request, so
//!   the command waits (bounded) for the player's `SessionConnected`, which
//!   sets the snapshot's `active`. If it never lands, `activate` takes over
//!   locally instead.
//! - **Local** (a load): `activate` alone. The Spirc handles its commands in
//!   order and `activate` sets it active at once, so the load that follows is
//!   not dropped.
//! - **IfActive** (pause, a duck): nothing at all while inactive.
//! - **Active** (add to queue): refused while inactive, saying why. The
//!   queue is the active device's: librespot's Spirc can add to its own
//!   alone, and has no command for another device's. Pulling playback here
//!   just to queue a track would move what is playing.

use std::time::Duration;

use tauri::AppHandle;

use super::state;

/// How long a pull waits for Spotify's transfer to reach this device.
const PULL_WAIT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Need {
    Pull,
    Local,
    IfActive,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    Send,
    Skip,
    ActivateThenSend,
    PullThenSend,
    Refuse,
}

pub(super) fn step(active: bool, need: Need) -> Step {
    match (active, need) {
        (true, _) => Step::Send,
        (false, Need::Pull) => Step::PullThenSend,
        (false, Need::Local) => Step::ActivateThenSend,
        (false, Need::IfActive) => Step::Skip,
        (false, Need::Active) => Step::Refuse,
    }
}

/// Why an `Active` command was refused, naming the device playing if known.
pub(super) fn refusal(playing_on: Option<&str>) -> String {
    match playing_on {
        Some(device) => {
            format!("Playing on {device}: Oculus can add to the queue only while it plays here")
        }
        None => "Play something here first, then add to the queue".into(),
    }
}

/// Runs `then` once this device can take it (see the module docs).
pub(super) async fn send(
    app: &AppHandle,
    need: Need,
    then: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let active = state::active(app).ok_or("Not connected to Spotify")?;
    match step(active, need) {
        Step::Send => {}
        Step::Skip => return Ok(()),
        Step::Refuse => return Err(refusal(state::snapshot(app).playing_on.as_deref())),
        Step::ActivateThenSend => state::with_spirc(app, |spirc| spirc.activate())?,
        Step::PullThenSend => pull(app).await?,
    }
    then()
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

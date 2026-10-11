//! Settings' side of the harness: where each CLI is, installing and updating
//! it, signing in, and the rate-limit meter.

use tauri::{AppHandle, Emitter, State};

use crate::harness::Provider;
use crate::harness::{discover, install, signin, update};
use crate::runtime::blocking::run as blocking;

use super::HarnessState;

/// Where each CLI is, and whether it is there at all. Only `recheck`
/// (Settings' *Recheck*) drops `discover`'s caches — a full recheck can
/// cost a login shell per provider, and every model picker reads this.
#[tauri::command]
pub async fn harness_health(recheck: bool) -> Vec<discover::BridgeHealth> {
    tokio::task::spawn_blocking(move || {
        if recheck {
            discover::forget();
        }
        discover::PROVIDERS
            .iter()
            .map(|p| discover::health(*p))
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// The ways this machine could install one of the CLIs, and which it can
/// run. Detection is blocking and cached (`discover::tool`).
#[tauri::command]
pub async fn harness_install_offer(provider: Provider) -> install::InstallOffer {
    tokio::task::spawn_blocking(move || install::offer(provider, install::detect()))
        .await
        // The only safe reading of a failed join: commands to copy, no buttons.
        .unwrap_or_else(|_| install::offer(provider, install::Managers::default()))
}

/// Run one route. The webview names a provider and a manager, never the
/// command. Output streams on `install::INSTALL_EVENT`; Settings rechecks
/// on the `done` event.
#[tauri::command]
pub async fn harness_install_run(
    app: AppHandle,
    provider: Provider,
    manager: install::Manager,
) -> Result<(), String> {
    blocking(move || {
        let emitter = app.clone();
        install::start(provider, manager, move |line| {
            emitter.emit(install::INSTALL_EVENT, line).ok();
        })
    })
    .await
}

/// Each installed CLI's version against its newest published one. The
/// registry answers are cached in Rust; `recheck` drops them.
#[tauri::command]
pub async fn harness_updates(recheck: bool) -> Vec<update::UpdateInfo> {
    tokio::task::spawn_blocking(move || {
        if recheck {
            update::forget();
        }
        update::check_all()
    })
    .await
    .unwrap_or_default()
}

/// Update one CLI with the command `harness_updates` showed. Output
/// streams on `update::UPDATE_EVENT`; one update runs at a time.
#[tauri::command]
pub async fn harness_update_run(app: AppHandle, provider: Provider) -> Result<(), String> {
    blocking(move || {
        let emitter = app.clone();
        update::start(provider, move |line| {
            emitter.emit(update::UPDATE_EVENT, line).ok();
        })
    })
    .await
}

/// Whether a provider has credentials, asked of the CLI every time (see
/// `cli/signin` for why this is never cached).
#[tauri::command]
pub async fn harness_sign_in_status(provider: Provider) -> signin::SignInStatus {
    tokio::task::spawn_blocking(move || signin::status(provider))
        .await
        .unwrap_or_else(|e| signin::SignInStatus {
            provider,
            signed_in: None,
            account: None,
            error: Some(e.to_string()),
        })
}

/// Run the provider's own login flow. Output streams on
/// `signin::SIGNIN_EVENT`; the first URL opens in the *system* browser,
/// where the student is already signed in, and rides the event too so
/// the dialog can offer it to copy.
#[tauri::command]
pub async fn harness_sign_in_start(app: AppHandle, provider: Provider) -> Result<(), String> {
    blocking(move || {
        let emitter = app.clone();
        signin::start(provider, move |line| {
            if let Some(url) = line.url.as_deref() {
                tauri_plugin_opener::open_url(url, None::<&str>).ok();
            }
            emitter.emit(signin::SIGNIN_EVENT, line).ok();
        })
    })
    .await
}

/// The code Claude's flow ends on, pasted back from the browser.
#[tauri::command]
pub async fn harness_sign_in_code(
    state: State<'_, HarnessState>,
    provider: Provider,
    code: String,
) -> Result<(), String> {
    let h = state.harness.clone();
    blocking(move || {
        let signed_in = signin::submit_code(provider, &code);
        if provider == Provider::Claude {
            h.forget_claude_models();
        }
        signed_in
    })
    .await
}

/// The student closed the dialog — the only thing that ends a login.
#[tauri::command]
pub async fn harness_sign_in_cancel(provider: Provider) -> Result<(), String> {
    blocking(move || signin::cancel(provider)).await
}

/// Only Codex can be asked for its plan windows; the other providers'
/// arrive with a turn.
#[tauri::command]
pub async fn harness_refresh_rate_limits(
    state: State<'_, HarnessState>,
    provider: Provider,
) -> Result<(), String> {
    if provider != Provider::Codex {
        return Ok(());
    }
    let h = state.harness.clone();
    blocking(move || {
        h.refresh_codex_rate_limits();
        Ok(())
    })
    .await
}

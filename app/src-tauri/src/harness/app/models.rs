//! Each provider's model catalogue and opencode's provider credentials.

use std::collections::BTreeMap;

use tauri::State;

use crate::harness::codex::ModelInfo;
use crate::harness::{antigravity, claude, opencode};
use crate::runtime::blocking::run as blocking;

use super::HarnessState;

#[tauri::command]
pub async fn harness_codex_models(
    state: State<'_, HarnessState>,
) -> Result<Vec<ModelInfo>, String> {
    let h = state.harness.clone();
    blocking(move || h.codex_models()).await
}

#[tauri::command]
pub async fn harness_antigravity_models(
    state: State<'_, HarnessState>,
) -> Result<Vec<antigravity::ModelInfo>, String> {
    let h = state.harness.clone();
    blocking(move || h.antigravity_models()).await
}

/// Claude Code's catalogue, off the CLI's `initialize` answer — no turn,
/// nothing billed.
#[tauri::command]
pub async fn harness_claude_models(
    state: State<'_, HarnessState>,
) -> Result<Vec<claude::ModelInfo>, String> {
    let h = state.harness.clone();
    blocking(move || h.claude_models()).await
}

/// opencode's catalogue. Starts the server if it is not up.
#[tauri::command]
pub async fn harness_opencode_models(
    state: State<'_, HarnessState>,
) -> Result<Vec<opencode::ModelInfo>, String> {
    let h = state.harness.clone();
    blocking(move || h.opencode_models()).await
}

/// opencode's catalogue if its server is up; `None` instead of starting it.
#[tauri::command]
pub async fn harness_opencode_models_if_running(
    state: State<'_, HarnessState>,
) -> Result<Option<Vec<opencode::ModelInfo>>, String> {
    let h = state.harness.clone();
    blocking(move || h.opencode_models_if_running()).await
}

// opencode's credentials: each starts the server if it is down, so none is
// called on a page opening. A credential never comes back out: there is no
// read side.

#[tauri::command]
pub async fn harness_opencode_providers(
    state: State<'_, HarnessState>,
    refresh: bool,
) -> Result<opencode::ProviderList, String> {
    let h = state.harness.clone();
    blocking(move || h.opencode_providers(refresh)).await
}

#[tauri::command]
pub async fn harness_opencode_set_key(
    state: State<'_, HarnessState>,
    provider: String,
    method: usize,
    key: String,
    answers: Option<BTreeMap<String, String>>,
) -> Result<opencode::ProviderList, String> {
    let h = state.harness.clone();
    let answers = answers.unwrap_or_default();
    blocking(move || h.opencode_set_api_key(&provider, method, &key, &answers)).await
}

#[tauri::command]
pub async fn harness_opencode_disconnect(
    state: State<'_, HarnessState>,
    provider: String,
) -> Result<opencode::ProviderList, String> {
    let h = state.harness.clone();
    blocking(move || h.opencode_disconnect(&provider)).await
}

/// Start a browser flow: the URL to open (in the system browser), whether
/// the server finishes it by itself (`auto`), and what to tell the student.
#[tauri::command]
pub async fn harness_opencode_oauth_start(
    state: State<'_, HarnessState>,
    provider: String,
    method: usize,
    answers: Option<BTreeMap<String, String>>,
) -> Result<opencode::Authorization, String> {
    let h = state.harness.clone();
    let answers = answers.unwrap_or_default();
    blocking(move || h.opencode_oauth_authorize(&provider, method, &answers)).await
}

/// Finish a `code` flow. An `auto` one never calls this; the dialog polls
/// `harness_opencode_providers` with `refresh` instead.
#[tauri::command]
pub async fn harness_opencode_oauth_finish(
    state: State<'_, HarnessState>,
    provider: String,
    method: usize,
    code: Option<String>,
) -> Result<opencode::ProviderList, String> {
    let h = state.harness.clone();
    blocking(move || h.opencode_oauth_callback(&provider, method, code.as_deref())).await
}

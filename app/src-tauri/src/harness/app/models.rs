//! Each provider's model catalogue, opencode's provider credentials and
//! Antigravity's approved rules.

use std::collections::BTreeMap;

use tauri::State;

use crate::harness::codex::ModelInfo;
use crate::harness::Provider;
use crate::harness::{antigravity, antigravity_rules, claude, opencode, store};
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

/// Allow what an Antigravity thread was just stopped at. `rule` is in
/// `agy`'s syntax. It is stored and the thread's process dropped (a live
/// `agy` never re-reads its rules); the webview sends the follow-up.
/// Refused mid-turn, and for a rule one of Oculus's own denies covers.
#[tauri::command]
pub async fn harness_antigravity_allow(
    state: State<'_, HarnessState>,
    thread_id: i64,
    rule: String,
) -> Result<Vec<String>, String> {
    let rule = rule.trim().to_string();
    if !antigravity_rules::is_valid_rule(&rule) {
        return Err(format!(
            "not a rule Oculus can allow: {rule:?} — expected command(…), read_file(…), \
             write_file(…) or read_url(…)"
        ));
    }
    if let Some(d) = antigravity_rules::denied_by(&crate::library::paths::data_dir(), &rule) {
        return Err(format!(
            "{rule} would change nothing: Oculus keeps {d} closed to every agent"
        ));
    }
    let pool = crate::db::store::open_pool().await?;
    let row = store::thread(&pool, thread_id).await?;
    if row.provider != Provider::Antigravity {
        return Err(format!(
            "thread {thread_id} is a {} thread",
            row.provider.label()
        ));
    }
    if state.queue.lock().unwrap().is_busy(thread_id) {
        return Err("stop the current turn before allowing something new".into());
    }
    let mut rules = antigravity_rules::stored(&pool).await?;
    if !rules.contains(&rule) {
        rules.push(rule);
        antigravity_rules::save(&pool, &rules).await?;
    }
    state.harness.close(thread_id);
    Ok(rules)
}

/// The student's Antigravity approvals, for Settings to list.
#[tauri::command]
pub async fn harness_antigravity_rules() -> Result<Vec<String>, String> {
    let pool = crate::db::store::open_pool().await?;
    antigravity_rules::stored(&pool).await
}

/// Take an approval back. The settings file is rewritten now (the
/// student's own `agy` reads it too), and every idle Antigravity thread's
/// process is dropped so none keeps running under the rule.
#[tauri::command]
pub async fn harness_antigravity_revoke(
    state: State<'_, HarnessState>,
    rule: String,
) -> Result<Vec<String>, String> {
    let pool = crate::db::store::open_pool().await?;
    let mut rules = antigravity_rules::stored(&pool).await?;
    rules.retain(|r| r != rule.trim());
    antigravity_rules::save(&pool, &rules).await?;
    let written = rules.clone();
    blocking(move || antigravity_rules::install(&crate::library::paths::data_dir(), Some(written)))
        .await?;
    for id in state.harness.live_threads(Provider::Antigravity) {
        if !state.queue.lock().unwrap().is_busy(id) {
            state.harness.close(id);
        }
    }
    Ok(rules)
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

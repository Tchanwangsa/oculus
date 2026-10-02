//! Tauri commands for the embedding settings — Settings → Library.
//!
//! One engine is selected; every stored vector came from it and queries are
//! embedded by it — no fallback, no fusing of spaces. Changing the engine
//! therefore discards the index in the same call (see `embed_set_engine`).

use std::path::Path;

use serde::Serialize;
use sqlx::Row;

use super::estimate::EmbedEstimate;
use super::voyage::ledger::{self, UsageLedger};
use super::{Engine, EMBED_DIM, EMBED_MODEL};
use crate::retrieval::IndexStats;
use crate::store::{db_path, edit_setting, pool};

/// The `settings` row this module writes and `embed::embed_config` reads.
const SETTINGS_KEY: &str = "embed";

/// Is there a backend behind `Engine::Local` in this build? No: the local
/// embedder is a separate program with no client here yet, so the option is
/// shown disabled with a reason.
const LOCAL_READY: bool = false;

/// Shown under the disabled option, and as the refusal if it is chosen anyway.
const LOCAL_UNAVAILABLE: &str =
    "The local embedder is a separate program that runs on this Mac, and Oculus does not ship \
     one yet.";

// ── The view ─────────────────────────────────────────────────────────────────

/// One selectable backend. Labels live beside the refusal so they cannot drift.
#[derive(Serialize)]
pub struct EngineOption {
    /// The value `embed_set_engine` takes, and what lands in the settings row.
    pub id: &'static str,
    pub label: &'static str,
    /// Where the embedding happens, in one line. Always shown.
    pub detail: &'static str,
    pub available: bool,
    /// Why not — `None` whenever `available` is true.
    pub unavailable_reason: Option<&'static str>,
}

/// What the account has spent, its programme, and the spend guard.
///
/// Oculus's own count (`voyage-usage.json`), not Voyage's books: Voyage has no
/// usage endpoint. Pessimistic by construction, so it drifts high.
#[derive(Serialize)]
pub struct VoyageUsage {
    /// `"free"`, `"paid"` or `"unknown"` (the tier is still the opening guess).
    pub plan: &'static str,
    /// `ledger::TierSource`: how much `plan` is worth.
    pub plan_source: &'static str,
    pub rpm: f64,
    pub tpm: f64,
    /// Unix seconds the limits above were last learned.
    pub learned_at: u64,

    /// Cumulative, for the life of the account as this app has seen it.
    pub requests: u64,
    pub tokens: u64,
    pub pixels: u64,
    /// The grant every account gets, and what is left of it.
    pub free_pixels: u64,
    pub free_pixels_left: u64,
    pub usd_per_billion_pixels: f64,

    /// The spend guard: stop at this percentage of the grant. 0 is off.
    pub stop_at_percent: u8,
    /// Voyage itself said the allowance is gone, and the latch has not expired.
    pub quota_latched: bool,
}

fn usage_view() -> VoyageUsage {
    let usage = UsageLedger::shared().snapshot();
    VoyageUsage {
        // Never "free" over an opening assumption.
        plan: match (usage.tier.source, usage.tier.is_free()) {
            (ledger::TierSource::Assumed, _) => "unknown",
            (_, true) => "free",
            (_, false) => "paid",
        },
        plan_source: usage.tier.source.as_str(),
        rpm: usage.tier.rpm,
        tpm: usage.tier.tpm,
        learned_at: usage.tier.learned_at,
        requests: usage.requests,
        tokens: usage.tokens,
        pixels: usage.pixels,
        free_pixels: ledger::FREE_PIXELS,
        free_pixels_left: ledger::FREE_PIXELS.saturating_sub(usage.pixels),
        usd_per_billion_pixels: ledger::USD_PER_BILLION_PIXELS,
        stop_at_percent: usage.stop_at_percent.min(100),
        quota_latched: usage.latched(),
    }
}

/// Everything Settings → Library needs to draw the embedding control and the
/// consequence of changing it.
#[derive(Serialize)]
pub struct EmbedSettings {
    /// The selected engine: `"cloud"` or `"local"`.
    pub engine: &'static str,
    /// The API root in force, default or overridden.
    pub base_url: String,
    /// The space this app writes into and searches.
    pub model: &'static str,
    pub dim: usize,
    /// Cloud: a key is in the keychain. Local: always true.
    pub credentials_ready: bool,
    pub engines: Vec<EngineOption>,
    /// What is in the index *now* — the number the confirmation quotes.
    pub index: IndexStats,
    /// The account behind the selected engine; `None` for a local engine.
    pub usage: Option<VoyageUsage>,
}

fn engines() -> Vec<EngineOption> {
    vec![
        EngineOption {
            id: Engine::Cloud.as_str(),
            label: "Voyage",
            detail: "Pages are rendered here and sent to Voyage AI to be embedded.",
            available: true,
            unavailable_reason: None,
        },
        EngineOption {
            id: Engine::Local.as_str(),
            label: "Local server",
            detail: "Pages are embedded by a server running on this Mac. Nothing leaves it.",
            available: LOCAL_READY,
            unavailable_reason: (!LOCAL_READY).then_some(LOCAL_UNAVAILABLE),
        },
    ]
}

fn available(engine: Engine) -> bool {
    match engine {
        Engine::Cloud => true,
        Engine::Local => LOCAL_READY,
    }
}

async fn view(db: &Path) -> Result<EmbedSettings, String> {
    let config = super::embed_config();
    Ok(EmbedSettings {
        engine: config.engine.as_str(),
        base_url: config.base_url,
        model: EMBED_MODEL,
        dim: EMBED_DIM,
        credentials_ready: match config.engine {
            Engine::Cloud => crate::voyage::stored_api_key().is_some(),
            Engine::Local => true,
        },
        engines: engines(),
        index: crate::retrieval::stats(db).await?,
        usage: match config.engine {
            Engine::Cloud => Some(usage_view()),
            Engine::Local => None,
        },
    })
}

/// Read the current selection, the engines on offer, and the index it would
/// cost to change.
#[tauri::command]
pub async fn embed_settings() -> Result<EmbedSettings, String> {
    view(&db_path()).await
}

/// Move the spend guard (percent of the free pixel grant; 0 is off) and hand
/// back the settings so the page redraws from one answer. Stored in the ledger:
/// see `ledger::Usage::stop_at_percent`.
#[tauri::command]
pub async fn embed_set_budget(percent: u8) -> Result<EmbedSettings, String> {
    UsageLedger::shared().store_stop_at(percent);
    view(&db_path()).await
}

/// Is something account-wide stopping the run? The message, or `None` when the
/// next file may go. Lets the index loop stop once instead of failing every
/// remaining file. Only the ledger's latches: no client, no key, no request.
#[tauri::command]
pub fn embed_blocked() -> Option<String> {
    match super::embed_config().engine {
        Engine::Cloud => UsageLedger::shared().ensure_available(0).err().map(|e| e.to_string()),
        Engine::Local => None,
    }
}

/// What the outstanding run would cost and how long it would take. Separate
/// from `embed_settings` because it opens every outstanding PDF; blocking
/// because pdfium is synchronous.
#[tauri::command]
pub async fn embed_estimate() -> Result<EmbedEstimate, String> {
    let database = db_path();
    let base = crate::paths::data_dir();
    crate::blocking::run(move || {
        tauri::async_runtime::block_on(super::estimate::estimate(&database, &base))
    })
    .await
}

// ── Changing it ──────────────────────────────────────────────────────────────

/// Select an embedding backend, and discard the index when that is a change:
/// two spaces in one table rank noise (see `Health::check`).
///
/// The order is the safety argument — a failure can leave an index to rebuild,
/// never a setting naming one space over a table holding another:
///
/// 1. The `.emb.json` records, which `is_embedded` would otherwise skip on.
/// 2. The table, in one transaction: vectors and per-file `embed_status`.
/// 3. The setting, last.
#[tauri::command]
pub async fn embed_set_engine(engine: String) -> Result<EmbedSettings, String> {
    let chosen = match engine.trim() {
        "cloud" => Engine::Cloud,
        "local" => Engine::Local,
        other => return Err(format!("not an embedding engine: {other}")),
    };
    if !available(chosen) {
        return Err(match chosen {
            Engine::Local => LOCAL_UNAVAILABLE.to_string(),
            Engine::Cloud => unreachable!("cloud is always available"),
        });
    }

    let database = db_path();
    if super::embed_config().engine == chosen {
        // A stray re-selection must not trigger a re-index.
        return view(&database).await;
    }

    let base = crate::paths::data_dir();
    let db = pool(&database).await?;

    // 1. The records beside the PDFs.
    for relative in embeddable_paths(&db).await? {
        let Some(pdf_rel) = crate::paths::doc_pdf_rel(&relative) else {
            continue;
        };
        let record = super::emb_path(&base.join(pdf_rel));
        match std::fs::remove_file(&record) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                db.close().await;
                return Err(format!("could not clear {}: {error}", record.display()));
            }
        }
    }

    // 2. The table.
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE pages
            SET embedding = NULL, embed_model = NULL, embed_dim = NULL, embedded_at = NULL
          WHERE embedding IS NOT NULL",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("could not clear the page vectors: {e}"))?;
    // The page markdown stays; it is independent of the model.
    sqlx::query("UPDATE files SET embed_status = NULL, embedded_at = NULL WHERE embed_status IS NOT NULL")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("could not reset the index state: {e}"))?;
    tx.commit().await.map_err(|e| e.to_string())?;

    // 3. The setting.
    let result = write_engine(&db, chosen).await;
    db.close().await;
    result?;

    view(&database).await
}

/// Every library file that could have a `.emb.json` beside it — PDFs and
/// Office documents with a PDF sibling; the index queue's predicate.
async fn embeddable_paths(db: &sqlx::SqlitePool) -> Result<Vec<String>, String> {
    let rows = sqlx::query(
        "SELECT relative_path FROM files
          WHERE lower(file_type) IN ('pdf', 'pptx', 'docx', 'ppt', 'doc')",
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows.iter().filter_map(|row| row.try_get::<String, _>("relative_path").ok()).collect())
}

/// Write the engine into the `embed` row, editing the shared JSON blob in
/// place. `engineUrl` is dropped: it pointed at the old engine.
async fn write_engine(db: &sqlx::SqlitePool, engine: Engine) -> Result<(), String> {
    edit_setting(db, SETTINGS_KEY, |object| {
        object.insert("engine".into(), serde_json::Value::String(engine.as_str().into()));
        object.remove("engineUrl");
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_engine_in_the_seam_has_a_row() {
        // A new arm of `Engine` that nobody added an option for would be a
        // backend the settings page cannot select or explain.
        let options = engines();
        for engine in [Engine::Cloud, Engine::Local] {
            assert!(
                options.iter().any(|option| option.id == engine.as_str()),
                "no settings row for {}",
                engine.as_str()
            );
        }
    }

    #[test]
    fn an_unavailable_engine_always_says_why() {
        for option in engines() {
            assert_eq!(
                option.available,
                option.unavailable_reason.is_none(),
                "{} is inconsistent about its availability",
                option.id
            );
        }
    }
}

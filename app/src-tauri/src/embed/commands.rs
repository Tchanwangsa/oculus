//! Tauri commands for the embedding settings — Settings → Library.
//!
//! One backend is *selected*, and search runs against that one. There is no
//! automatic fallback between engines and no fusing of two spaces at query
//! time: `embed_config()` names one engine, every page vector in the `pages`
//! table came from it, and a query is embedded by the same one.
//!
//! That is what makes this a settings page with teeth rather than a dropdown.
//! **Changing the engine invalidates every stored vector**, because the two
//! spaces are not comparable — `Health::check` refuses a mismatch precisely
//! because mixing them yields confident, well-formatted, meaningless
//! rankings. So `embed_set_engine` does not just write a row: it throws the
//! index away in the same call, and the UI has to have said so first.
//!
//! The order of operations in `embed_set_engine` is the whole safety
//! argument, and it is written out at the call site: the setting is the *last*
//! thing to move, so a failure anywhere can leave the app with an index it
//! must rebuild, but never with a setting that claims one space while the
//! table holds another.

use std::path::Path;

use serde::Serialize;
use sqlx::Row;
use tauri::{AppHandle, Manager};

use super::estimate::EmbedEstimate;
use super::voyage::ledger::{self, UsageLedger};
use super::{Engine, EMBED_DIM, EMBED_MODEL};
use crate::retrieval::IndexStats;
use crate::store::{db_path, pool};

/// The `settings` row this module owns. Shared with `embed::embed_config`,
/// which reads it; nothing else writes it.
const SETTINGS_KEY: &str = "embed";

/// Is there a backend behind `Engine::Local` in this build?
///
/// **No, and the constant is the honest way to say so.** The local arm of the
/// seam is real — the enum has it, `embed_config` resolves a base URL and a
/// credential source for it, and this command will write it — but the program
/// it talks to over loopback on 9548 is a separate repo that does not ship
/// with the app. So the option is offered and disabled with a reason, rather
/// than hidden (which would misrepresent the architecture) or left live
/// (which would be a control that points the indexer at a closed port).
///
/// When that server lands, this flips to `true` and the seam gains a local
/// `Embedder`; nothing else on this path changes.
const LOCAL_READY: bool = false;

/// What a student is told when they reach for the option they cannot have.
/// One sentence, naming the reason rather than a ticket, and written so it
/// reads on its own line under the control.
const LOCAL_UNAVAILABLE: &str =
    "The local embedder is a separate program that runs on this Mac, and Oculus does not ship \
     one yet.";

// ── The view ─────────────────────────────────────────────────────────────────

/// One selectable backend, with everything the row needs to draw itself.
///
/// The labels and the reason live in Rust rather than in the page so that the
/// thing which refuses an engine and the thing which explains the refusal
/// cannot drift apart.
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

/// What the account has spent, what programme it turned out to be on, and
/// where the spend guard sits.
///
/// **Every number here is Oculus's own count, not Voyage's books.** Voyage
/// publishes no usage endpoint — the dashboard is the only place the real
/// figure lives — so this is `voyage-usage.json`: what this app reserved before
/// each request, settled upwards against the `usage.total_tokens` every
/// response carries. It is deliberately pessimistic (a request that failed
/// uncertainly still counts), so it drifts high rather than low, and the page
/// that shows it says whose count it is.
#[derive(Serialize)]
pub struct VoyageUsage {
    /// `"free"`, `"paid"` or `"unknown"` — the account's programme as far as
    /// the rate-limit detector has got. `"unknown"` is an honest state and not
    /// a failure: the tier opens at an optimistic guess and is corrected by the
    /// first request or two, so a library that has never been indexed has never
    /// had the chance to find out.
    pub plan: &'static str,
    /// How much `plan` is worth: `stated` is Voyage's own words in a 429 body,
    /// `observed` is inferred from behaviour, `assumed` is the opening guess.
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
        // A free programme is a *fact about the account* when Voyage stated it
        // and a guess otherwise, so the two travel together and the page is
        // never allowed to print "Free" over an opening assumption.
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
    /// The space this app writes into and searches. Both halves, because
    /// "model changed" means nothing without the width beside it.
    pub model: &'static str,
    pub dim: usize,
    /// Cloud: a key is in the keychain. Local: nothing to authenticate, so
    /// this is true by construction.
    pub credentials_ready: bool,
    pub engines: Vec<EngineOption>,
    /// What is in the index *now* — the number the confirmation quotes.
    pub index: IndexStats,
    /// The account behind the selected engine. `None` for a local engine:
    /// there is no allowance, no tier and nothing to guard against.
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
pub async fn embed_settings(app: AppHandle) -> Result<EmbedSettings, String> {
    view(&db_path(&app)?).await
}

/// Move the spend guard, and hand back the settings so the page redraws from
/// one answer rather than from its own optimistic copy.
///
/// The percentage is of Voyage's free pixel grant, and 0 turns the guard off.
/// It lives in `voyage-usage.json` rather than in the `settings` row because
/// the reservation that enforces it already reads that file on every request:
/// one atomic read, and a process-wide singleton ledger that picks the change
/// up without being rebuilt. See `UsageLedger::budget`.
#[tauri::command]
pub async fn embed_set_budget(app: AppHandle, percent: u8) -> Result<EmbedSettings, String> {
    UsageLedger::shared().store_stop_at(percent);
    view(&db_path(&app)?).await
}

/// Is something account-wide stopping the run right now, and what is it?
///
/// The message when yes, `None` when the next file may go ahead. It exists
/// because the index loop is a loop: a spent allowance or a reached spend
/// limit condemns every remaining file for the same reason, and a run that
/// kept going would turn one fact into one error per file — 166 identical
/// lines, five of them shown, and a "stopped" that reads like a crash.
///
/// Only the ledger's two latches, deliberately. This is not a general health
/// check: it constructs no client, needs no key, costs no request, and answers
/// the one question the loop can act on between files. A local engine has no
/// allowance to be stopped by, so it is never blocked.
#[tauri::command]
pub fn embed_blocked() -> Option<String> {
    match super::embed_config().engine {
        Engine::Cloud => UsageLedger::shared().ensure_available(0).err().map(|e| e.to_string()),
        Engine::Local => None,
    }
}

/// What the outstanding run would cost and how long it would take.
///
/// Separate from `embed_settings` because it is **slow in a way the settings
/// are not** — it opens every outstanding PDF to read its page boxes — and the
/// page must be able to draw the rest of itself while this is still running.
///
/// `spawn_blocking` because pdfium is synchronous and a library-wide sweep on a
/// runtime worker would park the async scheduler for seconds.
#[tauri::command]
pub async fn embed_estimate(app: AppHandle) -> Result<EmbedEstimate, String> {
    let database = db_path(&app)?;
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(super::estimate::estimate(&database, &base))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Changing it ──────────────────────────────────────────────────────────────

/// Select an embedding backend, **and throw the index away** when that is a
/// change.
///
/// The discard is not a side effect to be tidied away later; it is what the
/// change *is*. Vectors from two models share a table, a width and a dot
/// product, and share no geometry at all — a search over the mixture returns
/// a confident ranking of unrelated pages, which is worse than an error
/// because nothing looks broken. So every vector goes, on the way through,
/// and the library is re-indexed against the new engine.
///
/// Three steps, in this order, and the order is the safety argument:
///
/// 1. **The on-disk records first.** `<stem>.emb.json` is what `is_embedded`
///    reads to skip a file; leave one behind and the re-index skips the very
///    page it exists to redo. Failing here leaves the old vectors *and* the
///    old setting intact, which is a consistent state.
/// 2. **Then the table**, in one transaction: the vectors and the per-file
///    `embed_status` that says they are there.
/// 3. **The setting last.** If this fails the app still names the old engine
///    with an empty index — an afternoon of re-indexing, not a corrupt one.
///    Written first, a failure at step 2 would leave the setting claiming one
///    space while the table held another, which is the one outcome that must
///    be impossible.
#[tauri::command]
pub async fn embed_set_engine(app: AppHandle, engine: String) -> Result<EmbedSettings, String> {
    let chosen = match engine.trim() {
        "cloud" => Engine::Cloud,
        "local" => Engine::Local,
        other => return Err(format!("not an embedding engine: {other}")),
    };
    if !available(chosen) {
        // The same sentence the row is disabled with, so a request that got
        // past a stale UI is refused in the words the UI would have used.
        return Err(match chosen {
            Engine::Local => LOCAL_UNAVAILABLE.to_string(),
            Engine::Cloud => unreachable!("cloud is always available"),
        });
    }

    let database = db_path(&app)?;
    if super::embed_config().engine == chosen {
        // Re-selecting what is already selected costs nothing. Clearing here
        // would turn a stray click into a re-index.
        return view(&database).await;
    }

    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
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
    // The markdown in `pages` stays: it is the citation substrate and the file
    // viewer's per-page source, and it has nothing to do with the model.
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

/// Every library file that could have a `.emb.json` beside it — PDFs and the
/// Office documents that get a converted PDF sibling. The same predicate the
/// index queue uses, so nothing it would re-embed is left holding a record.
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

/// Write the engine into the `embed` row without disturbing the rest of it.
///
/// The row is a shared blob — this seam reads two keys out of it and has no
/// opinion on anything else in there — so it is edited as JSON rather than
/// replaced. `engineUrl` is the exception and is deliberately dropped: an
/// override points at one engine's API, and carrying it across a switch would
/// silently aim the new engine at the old one's address.
async fn write_engine(db: &sqlx::SqlitePool, engine: Engine) -> Result<(), String> {
    let stored = sqlx::query("SELECT value FROM settings WHERE key = ?1")
        .bind(SETTINGS_KEY)
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?
        .map(|row| row.get::<String, _>("value"));

    let mut value = stored
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    let object = value.as_object_mut().ok_or("embed settings are not an object")?;
    object.insert("engine".into(), serde_json::Value::String(engine.as_str().into()));
    object.remove("engineUrl");

    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(SETTINGS_KEY)
    .bind(serde_json::to_string(&value).map_err(|e| e.to_string())?)
    .execute(db)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
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

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
    })
}

/// Read the current selection, the engines on offer, and the index it would
/// cost to change.
#[tauri::command]
pub async fn embed_settings(app: AppHandle) -> Result<EmbedSettings, String> {
    view(&db_path(&app)?).await
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

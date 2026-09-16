//! Tauri commands for the parser settings — Settings → Library.
//!
//! Deliberately the same shape as `embed/commands.rs`, one field over: the
//! same `EngineOption` row, the same rule that a label and a refusal both come
//! from Rust so the page cannot explain a refusal in different words than Rust
//! would use, the same tests at the bottom. Two settings sections that behaved
//! differently would be worse than either behaviour alone.
//!
//! **The symmetry is the control, not the consequence.** What is not ported is
//! the destruction. `embed_set_engine` throws the whole index away in the same
//! call, because vectors from two models share a table, a width and a dot
//! product and share no geometry at all — a search over the mixture returns a
//! confident ranking of unrelated pages. Nothing of the sort is true here:
//! markdown from a local MinerU and markdown from MinerU's cloud are the same
//! artifact, written at the same `PARSER_VERSION` with the same `mode`, and
//! `render` — the module that decides what the markdown says — is shared
//! between them by construction. So switching the parser re-parses nothing,
//! invalidates nothing and needs no confirmation dialog. If a future backend
//! ever does change the artifacts, `PARSER_VERSION` is the thing that moves,
//! and it moves for every backend at once.
//!
//! **Both engines are always `available`.** Availability here is not a
//! reachability question, and making it one would be a quiet trap: a student
//! has to be able to *select* Local and then go start their server, and a
//! server that is merely stopped must not read as an engine that was never
//! chosen. The three-state answer about what is actually listening lives in
//! `LocalProbe`, which is a live status line beside the endpoint field — not a
//! gate in front of the choice.

use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::Row;
use tauri::AppHandle;

use super::mineru::local::{self, LocalHealth};
use super::{parse_config, Engine, LOCAL_BASE_URL, PARSER_VERSION};
use crate::store::{db_path, pool};

/// The `settings` row this module owns. Shared with `parse::parse_config`,
/// which reads it; nothing else writes it.
const SETTINGS_KEY: &str = "parse";

// ── The view ─────────────────────────────────────────────────────────────────

/// One selectable backend, with everything the row needs to draw itself.
///
/// The labels and the reason live in Rust rather than in the page so that the
/// thing which refuses an engine and the thing which explains the refusal
/// cannot drift apart.
#[derive(Serialize)]
pub struct EngineOption {
    /// The value `parse_set_engine` takes, and what lands in the settings row.
    pub id: &'static str,
    pub label: &'static str,
    /// Where the parsing happens, in one line. Always shown.
    pub detail: &'static str,
    pub available: bool,
    /// Why not — `None` whenever `available` is true.
    pub unavailable_reason: Option<&'static str>,
}

/// Everything Settings → Library needs to draw the parser control.
///
/// No index statistics and no page count, unlike `EmbedSettings`: there is no
/// number here that changing the engine would cost.
#[derive(Serialize)]
pub struct ParseSettings {
    /// The selected engine: `"cloud"` or `"local"`.
    pub engine: &'static str,
    /// The API root in force, default or overridden.
    pub base_url: String,
    /// What `base_url` would be with no override — the endpoint field's
    /// placeholder, so the address a user reads and the address a parse uses
    /// are the same constant.
    pub default_base_url: &'static str,
    /// Is `base_url` an `engineUrl` override rather than the default? The
    /// field needs to know whether it is showing a value or a suggestion.
    pub overridden: bool,
    /// The artifact version both backends write. Shown because a version
    /// mismatch is one of the failures a student can be told about.
    pub parser_version: u32,
    /// Cloud: a token is in the keychain. Local: nothing to authenticate, so
    /// this is true by construction.
    pub credentials_ready: bool,
    pub engines: Vec<EngineOption>,
}

/// What is listening at a local address right now.
///
/// Three states, because "not reachable" would collapse two situations that
/// call for different actions: a server that is not running (start it) and a
/// MinerU that is running the V1 API (it has no `/file_parse` at all, so the
/// fix is a different server, not a different port).
#[derive(Serialize)]
pub struct LocalProbe {
    /// `"reachable"` | `"unreachable"` | `"version_mismatch"`.
    pub state: &'static str,
    pub base_url: String,
    pub backend: Option<String>,
    pub parser_version: Option<u32>,
    /// One sentence, always present unless the state is `"reachable"`.
    pub detail: Option<String>,
}

fn engines() -> Vec<EngineOption> {
    vec![
        EngineOption {
            id: Engine::Cloud.as_str(),
            label: "MinerU cloud",
            detail: "PDFs are uploaded to MinerU's service and parsed there.",
            available: true,
            unavailable_reason: None,
        },
        EngineOption {
            id: Engine::Local.as_str(),
            label: "Local server",
            detail: "PDFs are parsed by a MinerU server running on this Mac. Nothing leaves it.",
            available: true,
            unavailable_reason: None,
        },
    ]
}

fn view() -> ParseSettings {
    let config = parse_config();
    let overridden = super::stored_settings()
        .and_then(|stored| stored.engine_url)
        .is_some_and(|url| !url.trim().is_empty());
    ParseSettings {
        engine: config.engine.as_str(),
        base_url: config.base_url,
        default_base_url: config.engine.default_base_url(),
        overridden,
        parser_version: PARSER_VERSION,
        credentials_ready: match config.engine {
            Engine::Cloud => crate::mineru::stored_api_key().is_some(),
            Engine::Local => true,
        },
        engines: engines(),
    }
}

/// Read the current selection and the engines on offer.
#[tauri::command]
pub async fn parse_settings() -> Result<ParseSettings, String> {
    Ok(view())
}

// ── Changing it ──────────────────────────────────────────────────────────────

/// Select a parse backend.
///
/// That is the whole operation: a row is written and the next parse uses the
/// other backend. Nothing on disk is invalidated, because both backends write
/// the same artifacts — see this module's header for why that is a fact about
/// the design rather than a coincidence worth re-checking.
#[tauri::command]
pub async fn parse_set_engine(app: AppHandle, engine: String) -> Result<ParseSettings, String> {
    let chosen = match engine.trim() {
        "cloud" => Engine::Cloud,
        "local" => Engine::Local,
        other => return Err(format!("not a parse engine: {other}")),
    };
    if parse_config().engine == chosen {
        return Ok(view());
    }

    let db = pool(&db_path(&app)?).await?;
    let result = edit_settings(&db, |object| {
        object.insert("engine".into(), Value::String(chosen.as_str().into()));
        // An override points at one engine's API. Carried across a switch it
        // would silently aim the new engine at the old one's address — the
        // cloud root reached as if it were loopback, or the reverse.
        object.remove("engineUrl");
    })
    .await;
    db.close().await;
    result?;

    Ok(view())
}

/// Point the selected engine at a different address, or clear the override.
///
/// An empty or whitespace `url` **clears** it, so the field's "revert to
/// default" gesture is emptying it rather than retyping the default — which
/// would otherwise store a literal copy of a constant and pin it across a
/// release that moved it.
#[tauri::command]
pub async fn parse_set_engine_url(app: AppHandle, url: String) -> Result<ParseSettings, String> {
    let override_url = url.trim().trim_end_matches('/').to_string();
    if !override_url.is_empty() {
        // Checked here rather than at the first parse: a typo that is only
        // discovered when a document fails looks like a broken parser.
        let parsed = url::Url::parse(&override_url)
            .map_err(|_| "That is not an address — it needs to look like http://127.0.0.1:8000.")?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().unwrap_or_default().is_empty()
        {
            return Err("That is not an address — it needs to look like http://127.0.0.1:8000."
                .to_string());
        }
    }

    let db = pool(&db_path(&app)?).await?;
    let result = edit_settings(&db, |object| {
        if override_url.is_empty() {
            object.remove("engineUrl");
        } else {
            object.insert("engineUrl".into(), Value::String(override_url));
        }
    })
    .await;
    db.close().await;
    result?;

    Ok(view())
}

/// Ask what is listening, at `url` or at whatever is configured.
///
/// The optional URL is the point: the endpoint field has to be testable
/// *before* it is saved, or the only way to find out an address is wrong is to
/// commit to it first.
#[tauri::command]
pub async fn parse_probe_local(url: Option<String>) -> Result<LocalProbe, String> {
    let base = match url.as_deref().map(str::trim).filter(|candidate| !candidate.is_empty()) {
        Some(candidate) => candidate.trim_end_matches('/').to_string(),
        None => configured_local_url(),
    };

    // A blocking loopback request, off the runtime's workers: seconds here
    // would be seconds no other command could run.
    let address = base.clone();
    let state = tokio::task::spawn_blocking(move || local::probe(&address))
        .await
        .map_err(|e| e.to_string())?;

    Ok(match state {
        LocalHealth::Ready => LocalProbe {
            state: "reachable",
            base_url: base,
            backend: Some(local::BACKEND.to_string()),
            parser_version: Some(PARSER_VERSION),
            detail: None,
        },
        LocalHealth::NotServing => LocalProbe {
            state: "unreachable",
            detail: Some(format!(
                "A server answered at {base} but is not accepting work yet — give it a moment \
                 while it loads its models."
            )),
            base_url: base,
            backend: None,
            parser_version: None,
        },
        // Named rather than lumped in with "nothing there": this server is
        // running and healthy, and still cannot parse for us.
        LocalHealth::WrongApi => LocalProbe {
            state: "version_mismatch",
            detail: Some(format!(
                "The server at {base} speaks MinerU's V1 API, which has no file_parse endpoint. \
                 Oculus needs a MinerU 3.x server."
            )),
            base_url: base,
            backend: None,
            parser_version: None,
        },
        LocalHealth::Unreachable => LocalProbe {
            state: "unreachable",
            detail: Some(format!(
                "Nothing answered at {base}. Start the MinerU server, then test again."
            )),
            base_url: base,
            backend: None,
            parser_version: None,
        },
    })
}

/// The address a local parse would use *now*.
///
/// An `engineUrl` override only ever belongs to the selected engine —
/// `parse_set_engine` drops it on a change for exactly this reason — so on a
/// cloud install `base_url` is MinerU's public root, and probing that as if it
/// were loopback would report nonsense about a service nobody is running here.
fn configured_local_url() -> String {
    let config = parse_config();
    match config.engine {
        Engine::Local => config.base_url,
        Engine::Cloud => LOCAL_BASE_URL.to_string(),
    }
}

/// Edit the `parse` row without disturbing the rest of it.
///
/// The row is a shared blob: this seam reads two keys out of it and still
/// carries two dead ones from the Python sidecar (`memoryCapMb` and the legacy
/// `backend`), left there deliberately rather than migrated out. Writing a
/// fresh object would delete them, so the value is read, edited and written
/// back — the same shape as `embed/commands.rs`'s `write_engine`, for the same
/// reason.
async fn edit_settings(
    db: &sqlx::SqlitePool,
    edit: impl FnOnce(&mut Map<String, Value>),
) -> Result<(), String> {
    let stored = sqlx::query("SELECT value FROM settings WHERE key = ?1")
        .bind(SETTINGS_KEY)
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?
        .map(|row| row.get::<String, _>("value"));

    let mut value = stored
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    let object = value.as_object_mut().ok_or("parse settings are not an object")?;
    edit(object);

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

    /// Pinned because it looks like an oversight and is not. Selection is not
    /// gated on reachability: the order of operations for a new local server
    /// is choose it, then start it, and an engine greyed out until its server
    /// answers cannot be chosen first. `parse_probe_local` is where "is it
    /// running" is answered.
    #[test]
    fn both_engines_can_be_selected_whatever_is_running() {
        assert!(engines().iter().all(|option| option.available));
    }

    /// Every default has to be an address this client can actually reach, or
    /// the first run of a fresh install fails on a value nobody typed.
    #[test]
    fn each_engine_defaults_to_a_usable_root() {
        for engine in [Engine::Cloud, Engine::Local] {
            let default = engine.default_base_url();
            let parsed = url::Url::parse(default).expect(default);
            assert!(matches!(parsed.scheme(), "http" | "https"), "{default}");
            assert!(parsed.host_str().is_some(), "{default}");
        }
        // MinerU's own server binds this by default, and a default of ours it
        // does not answer on is a setting every user has to change first.
        assert_eq!(Engine::Local.default_base_url(), "http://127.0.0.1:8000");
    }
}

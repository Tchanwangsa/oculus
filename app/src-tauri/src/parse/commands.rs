//! Tauri commands for the parser settings — Settings → Library. The same
//! shape as `embed/commands.rs`: labels and refusals come from Rust.
//!
//! Unlike `embed_set_engine`, switching invalidates nothing: both engines
//! write the same artifacts through the shared `render`.
//!
//! **Both engines are always `available`**: a student must be able to select
//! Local before starting their server. What is listening is `LocalProbe`'s
//! job, a status line beside the endpoint field — not a gate on the choice.

use serde::Serialize;
use serde_json::Value;

use super::mineru::local::{self, LocalHealth};
use super::mineru::result_tls::{self, CertState};
use super::{parse_config, Engine, LOCAL_BASE_URL, PARSER_VERSION};
use crate::db::store::{db_path, edit_setting, pool};

/// The `settings` row this module writes; `parse::parse_config` reads it.
const SETTINGS_KEY: &str = "parse";

/// One selectable backend, with everything the row needs to draw itself.
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
#[derive(Serialize)]
pub struct ParseSettings {
    /// The selected engine: `"cloud"` or `"local"`.
    pub engine: &'static str,
    /// The API root in force, default or overridden.
    pub base_url: String,
    /// What `base_url` would be with no override — the field's placeholder.
    pub default_base_url: &'static str,
    /// Is `base_url` an `engineUrl` override rather than the default?
    pub overridden: bool,
    /// The artifact version both backends write.
    pub parser_version: u32,
    /// Cloud: a token is saved, in keyd's vault or the keychain. Local:
    /// always true.
    pub credentials_ready: bool,
    /// Cloud: the keychain or oculus-keyd refused to say whether a token is
    /// saved, so `credentials_ready` is false without the token being missing.
    pub credentials_error: Option<String>,
    /// Download results through MinerU's expired CDN certificate (on unless
    /// turned off); see `mineru::result_tls`.
    pub accept_expired_result_cert: bool,
    pub engines: Vec<EngineOption>,
}

/// What is listening at a local address right now. A V1 MinerU is its own
/// state: the fix is a different server, not starting one.
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
    let (credentials_ready, credentials_error) = match config.engine {
        Engine::Cloud => match crate::providers::mineru::mineru_has_api_key() {
            Ok(saved) => (saved, None),
            Err(e) => (false, Some(e)),
        },
        Engine::Local => (true, None),
    };
    let overridden = super::config::stored_settings()
        .and_then(|stored| stored.engine_url)
        .is_some_and(|url| !url.trim().is_empty());
    ParseSettings {
        engine: config.engine.as_str(),
        base_url: config.base_url,
        default_base_url: config.engine.default_base_url(),
        overridden,
        parser_version: PARSER_VERSION,
        credentials_ready,
        credentials_error,
        accept_expired_result_cert: config.accept_expired_result_cert,
        engines: engines(),
    }
}

/// Read the current selection and the engines on offer.
#[tauri::command]
pub async fn parse_settings() -> Result<ParseSettings, String> {
    Ok(view())
}

/// Select a parse backend. Nothing on disk is invalidated (see module header).
#[tauri::command]
pub async fn parse_set_engine(engine: String) -> Result<ParseSettings, String> {
    let chosen = match engine.trim() {
        "cloud" => Engine::Cloud,
        "local" => Engine::Local,
        other => return Err(format!("not a parse engine: {other}")),
    };
    if parse_config().engine == chosen {
        return Ok(view());
    }

    let db = pool(&db_path()).await?;
    let result = edit_setting(&db, SETTINGS_KEY, |object| {
        object.insert("engine".into(), Value::String(chosen.as_str().into()));
        // An override belongs to one engine's API.
        object.remove("engineUrl");
    })
    .await;
    db.close().await;
    result?;

    Ok(view())
}

/// Point the selected engine at a different address. An empty `url` clears
/// the override rather than storing a copy of the default.
#[tauri::command]
pub async fn parse_set_engine_url(url: String) -> Result<ParseSettings, String> {
    let override_url = url.trim().trim_end_matches('/').to_string();
    if !override_url.is_empty() {
        // Checked now: a typo found at the first parse looks like a broken parser.
        let parsed = url::Url::parse(&override_url)
            .map_err(|_| "That is not an address — it needs to look like http://127.0.0.1:8000.")?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().unwrap_or_default().is_empty()
        {
            return Err(
                "That is not an address — it needs to look like http://127.0.0.1:8000.".to_string(),
            );
        }
    }

    let db = pool(&db_path()).await?;
    let result = edit_setting(&db, SETTINGS_KEY, |object| {
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

/// Allow or refuse result downloads through the CDN's expired certificate.
#[tauri::command]
pub async fn parse_set_accept_expired_result_cert(accept: bool) -> Result<ParseSettings, String> {
    let db = pool(&db_path()).await?;
    let result = edit_setting(&db, SETTINGS_KEY, |object| {
        object.insert("acceptExpiredResultCert".into(), Value::Bool(accept));
    })
    .await;
    db.close().await;
    result?;
    Ok(view())
}

/// What the result CDN's certificate looked like at the last download. With
/// `probe`, a bare TLS handshake first — no API call, so nothing is billed.
#[tauri::command]
pub async fn parse_result_cert(probe: Option<bool>) -> Result<CertState, String> {
    let accept = parse_config().accept_expired_result_cert;
    if !probe.unwrap_or(false) {
        return Ok(result_tls::state(accept));
    }
    tokio::task::spawn_blocking(move || result_tls::probe(accept))
        .await
        .map_err(|e| e.to_string())
}

/// Ask what is listening, at `url` (so the field is testable before it is
/// saved) or at whatever is configured.
#[tauri::command]
pub async fn parse_probe_local(url: Option<String>) -> Result<LocalProbe, String> {
    let base = match url
        .as_deref()
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
    {
        Some(candidate) => candidate.trim_end_matches('/').to_string(),
        None => configured_local_url(),
    };

    // Blocking, so off the runtime's workers.
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

/// The address a local parse would use *now*. On a cloud install `base_url`
/// is MinerU's public root, so the local default is probed instead.
fn configured_local_url() -> String {
    let config = parse_config();
    match config.engine {
        Engine::Local => config.base_url,
        Engine::Cloud => LOCAL_BASE_URL.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_engine_in_the_seam_has_a_row() {
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

    /// Not an oversight: see the module header.
    #[test]
    fn both_engines_can_be_selected_whatever_is_running() {
        assert!(engines().iter().all(|option| option.available));
    }

    #[test]
    fn each_engine_defaults_to_a_usable_root() {
        for engine in [Engine::Cloud, Engine::Local] {
            let default = engine.default_base_url();
            let parsed = url::Url::parse(default).expect(default);
            assert!(matches!(parsed.scheme(), "http" | "https"), "{default}");
            assert!(parsed.host_str().is_some(), "{default}");
        }
        // MinerU's own server binds this by default.
        assert_eq!(Engine::Local.default_base_url(), "http://127.0.0.1:8000");
    }
}

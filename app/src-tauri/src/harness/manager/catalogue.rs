//! The shared Codex and opencode servers, and what each CLI offers: model
//! catalogues, rate-limit windows and opencode's provider credentials.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::harness::antigravity;
use crate::harness::claude;
use crate::harness::codex::{CodexServer, CodexSpawn, ModelInfo};
use crate::harness::discover;
use crate::harness::opencode::{self, OpencodeServer, OpencodeSpawn, ProviderList};
use crate::harness::Provider;

use super::instructions::{instructions, thread_cwd};
use super::naming::NAMING_INSTRUCTIONS;
use super::raw_log::RawLog;
use super::Harness;

pub(super) struct ClaudeCatalogue {
    bin: PathBuf,
    modified: Option<std::time::SystemTime>,
    models: Vec<claude::ModelInfo>,
}

impl Harness {
    /// The shared Codex server, started on first use.
    pub(super) fn codex_server(&self) -> Result<Arc<CodexServer>, String> {
        let mut slot = self.codex.lock().unwrap();
        if let Some(s) = slot.as_ref().filter(|s| s.is_alive()) {
            return Ok(s.clone());
        }
        let bin = discover::binary(Provider::Codex)?;
        let server = CodexServer::spawn(CodexSpawn {
            bin,
            env: discover::child_env(),
            raw_log: RawLog::open(&self.data_dir, 0),
            account_sink: self.codex_account_sink.lock().unwrap().clone(),
        })?;
        *slot = Some(server.clone());
        // Seed the rate-limit meter now rather than at the first turn, off
        // the caller's thread (this runs inside the first send).
        {
            let s = server.clone();
            std::thread::spawn(move || {
                if let Err(e) = s.read_rate_limits() {
                    eprintln!("[oculus] codex rate limits: {e}");
                }
            });
        }
        Ok(server)
    }

    /// Re-read the windows on a server that is already up. Never starts one:
    /// a page visit is not a reason to spawn a CLI.
    pub fn refresh_codex_rate_limits(&self) {
        let server = {
            let slot = self.codex.lock().unwrap();
            slot.as_ref().filter(|s| s.is_alive()).cloned()
        };
        if let Some(s) = server {
            if let Err(e) = s.read_rate_limits() {
                eprintln!("[oculus] codex rate limits: {e}");
            }
        }
    }

    pub fn codex_models(&self) -> Result<Vec<ModelInfo>, String> {
        self.codex_server()?.list_models()
    }

    /// Antigravity's catalogue: one short-lived `agy models`, nothing spent.
    pub fn antigravity_models(&self) -> Result<Vec<antigravity::ModelInfo>, String> {
        let bin = discover::binary(Provider::Antigravity)?;
        antigravity::list_models(&bin, &discover::child_env())
    }

    /// Claude Code's catalogue, from a short-lived CLI run the way a thread's
    /// is (`claude::list_models`), cached per binary. The lock is held across
    /// the probe so concurrent callers start one CLI; failures are not cached.
    pub fn claude_models(&self) -> Result<Vec<claude::ModelInfo>, String> {
        let bin = discover::binary(Provider::Claude)?;
        let resolved = std::fs::canonicalize(&bin).unwrap_or_else(|_| bin.clone());
        let modified = std::fs::metadata(&resolved).and_then(|m| m.modified()).ok();
        let mut slot = self.claude_models.lock().unwrap();
        if let Some(c) = slot
            .as_ref()
            .filter(|c| c.bin == resolved && c.modified == modified)
        {
            return Ok(c.models.clone());
        }
        let cwd = thread_cwd(&self.data_dir);
        std::fs::create_dir_all(&cwd)
            .map_err(|e| format!("cannot create {}: {e}", cwd.display()))?;
        let models = claude::list_models(&bin, &cwd, &discover::child_env())?;
        *slot = Some(ClaudeCatalogue {
            bin: resolved,
            modified,
            models: models.clone(),
        });
        Ok(models)
    }

    /// A sign-in may change the account, and with it the catalogue.
    pub fn forget_claude_models(&self) {
        *self.claude_models.lock().unwrap() = None;
    }

    /// The shared opencode server, started on first use. Its config (brief
    /// and containment ruleset, `opencode::write_config`) is re-rendered
    /// before every start so neither is stale.
    pub(super) fn opencode_server(&self) -> Result<Arc<OpencodeServer>, String> {
        let mut slot = self.opencode.lock().unwrap();
        if let Some(s) = slot.as_ref().filter(|s| s.is_alive()) {
            return Ok(s.clone());
        }
        let bin = discover::binary(Provider::Opencode)?;
        let directory = thread_cwd(&self.data_dir);
        opencode::write_config(
            &directory,
            &self.data_dir,
            &instructions(&self.data_dir, None, None),
            &opencode::OneOffPrompts {
                naming: NAMING_INSTRUCTIONS,
                writer: crate::harness::suggest::INSTRUCTIONS,
                lecture_end: crate::lectures::lecture_end::INSTRUCTIONS,
            },
        )?;
        let server = OpencodeServer::spawn(OpencodeSpawn {
            bin,
            directory,
            env: discover::child_env(),
            raw_log: RawLog::open(&self.data_dir, 0),
            default_sink: self.opencode_default_sink.lock().unwrap().clone(),
        })?;
        *slot = Some(server.clone());
        Ok(server)
    }

    pub fn opencode_models(&self) -> Result<Vec<opencode::ModelInfo>, String> {
        self.opencode_server()?.list_models()
    }

    // opencode's credentials go through the server's own auth endpoints, so
    // a credential lands in opencode's store and nowhere else. Each starts the
    // server if it is down: they answer a button press, never a page opening.

    pub fn opencode_providers(&self, refresh: bool) -> Result<ProviderList, String> {
        let server = self.opencode_server()?;
        // Refused during a running turn: the list may then predate a
        // credential written since.
        let stale = refresh && !server.refresh();
        Ok(ProviderList {
            providers: server.list_providers()?,
            stale,
        })
    }

    /// The key is never stored or logged here.
    pub fn opencode_set_api_key(
        &self,
        provider: &str,
        method: usize,
        key: &str,
        answers: &BTreeMap<String, String>,
    ) -> Result<ProviderList, String> {
        let server = self.opencode_server()?;
        let spec = server.auth_method(provider, method)?;
        server.set_api_key(provider, key, &opencode::visible_answers(&spec, answers))?;
        self.opencode_providers(true)
    }

    pub fn opencode_disconnect(&self, provider: &str) -> Result<ProviderList, String> {
        self.opencode_server()?.remove_auth(provider)?;
        self.opencode_providers(true)
    }

    pub fn opencode_oauth_authorize(
        &self,
        provider: &str,
        method: usize,
        answers: &BTreeMap<String, String>,
    ) -> Result<opencode::Authorization, String> {
        let server = self.opencode_server()?;
        let spec = server.auth_method(provider, method)?;
        server.oauth_authorize(provider, method, &opencode::visible_answers(&spec, answers))
    }

    pub fn opencode_oauth_callback(
        &self,
        provider: &str,
        method: usize,
        code: Option<&str>,
    ) -> Result<ProviderList, String> {
        self.opencode_server()?
            .oauth_callback(provider, method, code)?;
        self.opencode_providers(true)
    }
}

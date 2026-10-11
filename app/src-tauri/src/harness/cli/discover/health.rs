//! What Settings shows per CLI: where it is and which version.

use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use super::{binary, override_env};
use crate::harness::event::Provider;

/// What Settings shows: found where, which version, and if not, why.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BridgeHealth {
    pub provider: Provider,
    pub label: &'static str,
    pub path: Option<String>,
    pub version: Option<String>,
    pub error: Option<String>,
    /// Which env var overrides discovery, for the Settings hint.
    pub override_env: &'static str,
}

pub(super) fn health_cache() -> &'static Mutex<std::collections::HashMap<Provider, BridgeHealth>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<Provider, BridgeHealth>>> =
        OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Drop one provider's cached `--version`, after an update replaced it.
pub fn forget_health(provider: Provider) {
    health_cache().lock().unwrap().remove(&provider);
}

/// Where each binary is, its version, or why not. Cached, because the model
/// picker reads it on every composer; [`forget`](super::forget) clears it.
pub fn health(provider: Provider) -> BridgeHealth {
    if let Some(h) = health_cache().lock().unwrap().get(&provider) {
        return h.clone();
    }
    let h = probe_health(provider);
    // Probed outside the lock: racing callers write the same answer rather than block.
    health_cache().lock().unwrap().insert(provider, h.clone());
    h
}

fn probe_health(provider: Provider) -> BridgeHealth {
    let mut h = BridgeHealth {
        provider,
        label: provider.label(),
        path: None,
        version: None,
        error: None,
        override_env: override_env(provider),
    };
    match binary(provider) {
        Ok(p) => {
            h.path = Some(p.display().to_string());
            match std::process::Command::new(&p).arg("--version").output() {
                Ok(out) if out.status.success() => {
                    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    // `2.1.267 (Claude Code)` / `codex-cli 0.153.4` — keep the number.
                    let num = v
                        .split_whitespace()
                        .find(|w| w.chars().next().map_or(false, |c| c.is_ascii_digit()))
                        .unwrap_or(&v);
                    h.version = Some(num.to_string());
                }
                Ok(out) => {
                    h.error = Some(format!(
                        "`--version` failed: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    ))
                }
                Err(e) => h.error = Some(format!("cannot run {}: {e}", p.display())),
            }
        }
        Err(e) => h.error = Some(e),
    }
    h
}

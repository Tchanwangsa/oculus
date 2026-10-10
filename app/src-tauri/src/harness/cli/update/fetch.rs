//! Where each install's newest version is published, and a cache of what the
//! endpoints answered.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::source::{effective_source, Source};
use super::version::read_version;
use crate::harness::event::Provider;

/// How an endpoint's body carries the version.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Body {
    /// The whole body is the version (`downloads.claude.ai`).
    Text,
    /// A JSON object's `version` (npm, Homebrew, Antigravity's manifest).
    JsonVersion,
}

/// Where the newest version for this install lives. `channel` is Claude's
/// `autoUpdatesChannel`; `arch` is `arm64` or `amd64`.
pub(super) fn latest_endpoint(
    provider: Provider,
    source: Source,
    channel: &str,
    arch: &str,
) -> (String, Body) {
    let npm = |pkg: &str| {
        (
            format!("https://registry.npmjs.org/{pkg}/latest"),
            Body::JsonVersion,
        )
    };
    let cask = |name: &str| {
        (
            format!("https://formulae.brew.sh/api/cask/{name}.json"),
            Body::JsonVersion,
        )
    };
    match (provider, effective_source(provider, source)) {
        (Provider::Claude, Source::Brew) => cask("claude-code"),
        (Provider::Claude, Source::Npm) => npm("@anthropic-ai/claude-code"),
        (Provider::Claude, _) => (
            format!("https://downloads.claude.ai/claude-code-releases/{channel}"),
            Body::Text,
        ),
        (Provider::Codex, Source::Brew) => cask("codex"),
        // The curl installer pulls the same release npm publishes.
        (Provider::Codex, _) => npm("@openai/codex"),
        // opencode's formula is in its own tap, which formulae.brew.sh does
        // not serve; every route ships the same GitHub release as npm.
        (Provider::Opencode, _) => npm("opencode-ai"),
        (Provider::Antigravity, _) => (
            format!(
                "https://antigravity-cli-auto-updater-974169037036.us-central1.run.app/manifests/darwin_{arch}.json"
            ),
            Body::JsonVersion,
        ),
    }
}

/// `stable` when `~/.claude/settings.json` asks for it, else `latest`.
pub(super) fn claude_channel(home: Option<&Path>) -> &'static str {
    let stable = home
        .and_then(|h| std::fs::read_to_string(h.join(".claude/settings.json")).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .is_some_and(|v| v.get("autoUpdatesChannel").and_then(|c| c.as_str()) == Some("stable"));
    if stable {
        "stable"
    } else {
        "latest"
    }
}

pub(super) fn arch() -> &'static str {
    if std::env::consts::ARCH == "x86_64" {
        "amd64"
    } else {
        "arm64"
    }
}

pub(super) fn fetch(url: &str, shape: Body) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    let body = agent
        .get(url)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("{url} answered {code}"),
            ureq::Error::Transport(t) => format!("could not reach {url}: {t}"),
        })?
        .into_string()
        .map_err(|e| format!("could not read {url}: {e}"))?;
    read_version(&body, shape)
}

const FRESH_FOR: Duration = Duration::from_secs(6 * 60 * 60);
/// A failed check is retried sooner, but not on every visit while offline.
const FAILURE_FRESH_FOR: Duration = Duration::from_secs(10 * 60);

/// Newest versions by endpoint URL. Only the registry's answer is cached;
/// the installed side comes from `discover::health` on every read.
fn latest_cache() -> &'static Mutex<HashMap<String, (Instant, Result<String, String>)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Result<String, String>)>>> =
        OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Drop the cached newest versions; `discover::forget` calls it.
pub fn forget() {
    latest_cache().lock().unwrap().clear();
}

pub(super) fn latest(url: &str, shape: Body) -> Result<String, String> {
    if let Some((at, hit)) = latest_cache().lock().unwrap().get(url) {
        let ttl = if hit.is_ok() {
            FRESH_FOR
        } else {
            FAILURE_FRESH_FOR
        };
        if at.elapsed() < ttl {
            return hit.clone();
        }
    }
    let got = fetch(url, shape);
    latest_cache()
        .lock()
        .unwrap()
        .insert(url.to_string(), (Instant::now(), got.clone()));
    got
}

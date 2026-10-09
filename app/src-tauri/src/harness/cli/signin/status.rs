//! Whether each CLI has credentials, asked of the CLI itself.

use std::process::{Command, Stdio};

use serde::Serialize;

use crate::harness::cli::discover;
use crate::harness::event::Provider;

/// Serialized camelCase, like every other type crossing the invoke boundary.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SignInStatus {
    pub provider: Provider,
    /// `None` when this provider's credentials are not answerable from here.
    pub signed_in: Option<bool>,
    /// What the CLI calls the account: "Claude subscription", "ChatGPT", …
    pub account: Option<String>,
    /// Why the probe could not answer at all (binary missing, spawn failed).
    pub error: Option<String>,
}

impl SignInStatus {
    fn unknown(provider: Provider) -> Self {
        SignInStatus {
            provider,
            signed_in: None,
            account: None,
            error: None,
        }
    }
}

/// Whether the provider has credentials, asked of the provider (blocking).
/// Every probe is read-only; each CLI says "no" differently — Claude in JSON,
/// Codex by exit status, Antigravity with a sentence.
pub fn status(provider: Provider) -> SignInStatus {
    if provider == Provider::Opencode {
        // Not "no": not answerable from here.
        return SignInStatus::unknown(provider);
    }
    if provider == Provider::Antigravity {
        return antigravity_status();
    }

    let bin = match discover::binary(provider) {
        Ok(p) => p,
        Err(e) => {
            return SignInStatus {
                error: Some(e),
                ..SignInStatus::unknown(provider)
            }
        }
    };

    let args: &[&str] = match provider {
        Provider::Claude => &["auth", "status", "--json"],
        Provider::Codex => &["login", "status"],
        Provider::Opencode | Provider::Antigravity => unreachable!("returned above"),
    };

    let out = Command::new(&bin)
        .args(args)
        .env_clear()
        .envs(discover::child_env())
        .stdin(Stdio::null())
        .output();

    let out = match out {
        Ok(o) => o,
        Err(e) => {
            return SignInStatus {
                error: Some(format!("cannot run {}: {e}", bin.display())),
                ..SignInStatus::unknown(provider)
            }
        }
    };

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    match provider {
        Provider::Claude => match parse_claude_status(&stdout) {
            Some((signed_in, account)) => SignInStatus {
                provider,
                signed_in: Some(signed_in),
                account,
                error: None,
            },
            // An older `claude` without `auth status --json`: say so, not "no".
            None => SignInStatus {
                error: Some(format!(
                    "`claude auth status --json` did not answer in JSON: {}",
                    stdout.trim()
                )),
                ..SignInStatus::unknown(provider)
            },
        },
        // The exit code says whether; the account line (`Logged in using
        // ChatGPT`) arrives on stderr, so both streams are read.
        Provider::Codex => SignInStatus {
            provider,
            signed_in: Some(out.status.success()),
            account: out
                .status
                .success()
                .then(|| codex_account(&stdout).or_else(|| codex_account(&stderr)))
                .flatten(),
            error: None,
        },
        Provider::Opencode | Provider::Antigravity => unreachable!("returned above"),
    }
}

/// Antigravity's answer, off `agy models` (there is no status subcommand).
/// A listing is a yes; `Please sign in…` (agy 1.2.9) is a no; anything else
/// is no answer either way.
fn antigravity_status() -> SignInStatus {
    let provider = Provider::Antigravity;
    let run = discover::binary(provider)
        .and_then(|bin| crate::harness::antigravity::run_models(&bin, &discover::child_env()));
    let run = match run {
        Ok(r) => r,
        Err(e) => {
            return SignInStatus {
                error: Some(e),
                ..SignInStatus::unknown(provider)
            }
        }
    };
    let said = format!("{}\n{}", run.stdout, run.stderr);
    let signed_in = if run.success {
        Some(true)
    } else if said.to_lowercase().contains("please sign in") {
        Some(false)
    } else {
        None
    };
    SignInStatus {
        provider,
        signed_in,
        account: None,
        error: signed_in.is_none().then(|| {
            let said = said.trim();
            if said.is_empty() {
                "`agy models` failed and said nothing".to_string()
            } else {
                said.to_string()
            }
        }),
    }
}

/// `{"loggedIn": false, "authMethod": "none", …}` → (signed in, account).
/// Parsed from the first `{` so a notice above the JSON does not read as
/// "no"; a missing `loggedIn` is `None`, not "no".
pub(super) fn parse_claude_status(stdout: &str) -> Option<(bool, Option<String>)> {
    let start = stdout.find('{')?;
    let v: serde_json::Value = serde_json::from_str(stdout[start..].trim()).ok()?;
    let signed_in = v.get("loggedIn")?.as_bool()?;
    let email = v
        .get("email")
        .and_then(|e| e.as_str())
        .filter(|e| !e.is_empty());
    let method = v.get("authMethod").and_then(|m| m.as_str()).unwrap_or("");
    Some((signed_in, claude_account(signed_in, email, method)))
}

/// What Claude calls the account: the email (it names *which* account), else
/// the sign-in method. The CLI spells the subscription method `claude.ai`.
fn claude_account(signed_in: bool, email: Option<&str>, method: &str) -> Option<String> {
    if !signed_in {
        return None;
    }
    if let Some(email) = email {
        return Some(email.to_string());
    }
    Some(
        match method {
            "claude.ai" | "claudeai" => "Claude subscription",
            "console" => "Anthropic Console",
            "apiKey" | "api_key" => "API key",
            "" | "none" => "Signed in",
            other => other,
        }
        .to_string(),
    )
}

/// `Logged in using ChatGPT` → `ChatGPT`; anything else keeps the whole line.
pub(super) fn codex_account(stdout: &str) -> Option<String> {
    let line = stdout.lines().find(|l| !l.trim().is_empty())?.trim();
    let account = match line.to_lowercase().find("using ") {
        Some(i) => line[i + "using ".len()..].trim(),
        None => line,
    };
    (!account.is_empty()).then(|| account.to_string())
}

//! Signing in to a CLI agent, from the error row that says you are not.
//!
//! [`is_auth_failure`] classifies a provider's error text; its lists stay
//! tight because a false "sign in again" is worse than a miss. [`start`] /
//! [`submit_code`] / [`cancel`] drive the CLI's own login flow as a
//! subprocess (the discovered binary with [`discover::child_env`], never a
//! shell), streamed like `install.rs`; nothing here holds a token. There is
//! no deadline — [`cancel`] ends a flow.
//!
//! `claude auth login` blocks reading a pasted code from stdin; `codex login`
//! finishes on its own loopback listener (port 1455). opencode signs in per
//! provider through its server (`harness_opencode_*`), so it is not here.
//! [`status`] is never cached: it is read where a stale "signed out" would
//! be the one answer that must not be wrong.

use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;

use super::discover;
use super::event::Provider;

// ── Status ───────────────────────────────────────────────────────────────────

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
        .and_then(|bin| super::antigravity::run_models(&bin, &discover::child_env()));
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
fn parse_claude_status(stdout: &str) -> Option<(bool, Option<String>)> {
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
fn codex_account(stdout: &str) -> Option<String> {
    let line = stdout.lines().find(|l| !l.trim().is_empty())?.trim();
    let account = match line.to_lowercase().find("using ") {
        Some(i) => line[i + "using ".len()..].trim(),
        None => line,
    };
    (!account.is_empty()).then(|| account.to_string())
}

// ── Classifying a failure ────────────────────────────────────────────────────

/// Phrases that mean "no usable credentials" whichever CLI said them. Whole
/// clauses, never words: `"auth"` alone would match `auth.rs` in a compile
/// error.
const SHARED: &[&str] = &[
    "oauth token has expired",
    "oauth session expired",
    "session expired",
    "not logged in",
    "not authenticated",
    "invalid api key",
    "authentication_error",
    "authentication failed",
    "invalid_grant",
];

const CLAUDE: &[&str] = &[
    "failed to authenticate",
    "please run /login",
    "run /login",
    "claude auth login",
    "credentials are invalid",
];

/// `"chatgpt account"` appears in happy status output too, so for Codex it
/// only counts beside one of [`CODEX_ACCOUNT_TROUBLE`].
const CODEX: &[&str] = &["codex login", "please sign in"];
const CODEX_ACCOUNT_TROUBLE: &[&str] = &[
    "expired", "not ", "no ", "missing", "invalid", "failed", "required", "sign in",
];

/// What an opencode provider answers when its key is wrong or absent.
const OPENCODE: &[&str] = &[
    "opencode auth login",
    "no credentials",
    "provider is not connected",
    "incorrect api key",
    "invalid x-api-key",
];

/// Antigravity signs in through Google and keeps the credential in the keyring.
const ANTIGRAVITY: &[&str] = &[
    "please sign in",
    "google sign-in",
    "sign in to antigravity",
    "not signed in",
    "no credentials found in keyring",
    "reauthenticate",
];

/// Whether `msg` is the provider saying it has no usable credentials — which
/// puts a Sign in button on the row, so the lists stay tight. `401` counts
/// only beside "auth": a byte count or a path can carry those digits.
pub fn is_auth_failure(provider: Provider, msg: &str) -> bool {
    let m = msg.to_lowercase();
    if SHARED.iter().any(|n| m.contains(n)) {
        return true;
    }
    let own = match provider {
        Provider::Claude => CLAUDE,
        Provider::Codex => CODEX,
        Provider::Opencode => OPENCODE,
        Provider::Antigravity => ANTIGRAVITY,
    };
    if own.iter().any(|n| m.contains(n)) {
        return true;
    }
    if provider == Provider::Codex
        && m.contains("chatgpt account")
        && CODEX_ACCOUNT_TROUBLE.iter().any(|w| m.contains(w))
    {
        return true;
    }
    m.contains("401") && (m.contains("unauth") || m.contains("auth"))
}

// ── Driving the flow ─────────────────────────────────────────────────────────

/// A login's output, one event per line, like `install::INSTALL_EVENT`.
pub const SIGNIN_EVENT: &str = "harness-signin";

/// What the webview gets while a login runs.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SignInLine {
    pub provider: Provider,
    /// One line of the child's output, stdout and stderr both.
    pub line: Option<String>,
    /// The authorize URL, emitted once, the first time a line carries one.
    pub url: Option<String>,
    /// The last event of a run.
    pub done: bool,
    pub ok: Option<bool>,
    pub status: Option<String>,
}

/// A login in flight: [`submit_code`] writes to its stdin, [`cancel`] kills it.
struct Run {
    child: Arc<Mutex<Child>>,
    /// `None` only if the pipe could not be taken.
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    /// Set by [`cancel`], so a killed child reports "cancelled", not a signal.
    cancelled: Arc<AtomicBool>,
}

/// One login at a time per provider: a second `codex login` would find port
/// 1455 taken, a second `claude auth login` would split one pasted code.
fn running() -> &'static Mutex<HashMap<Provider, Run>> {
    static R: OnceLock<Mutex<HashMap<Provider, Run>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

fn login_args(provider: Provider) -> Result<&'static [&'static str], String> {
    match provider {
        // No `--claudeai`: Console accounts use the same subcommand.
        Provider::Claude => Ok(&["auth", "login"]),
        Provider::Codex => Ok(&["login"]),
        Provider::Opencode => Err(
            "opencode signs in per provider, in Settings → AI → Providers, not through a login \
             command"
                .into(),
        ),
        // `agy` signs in only interactively, in a terminal.
        Provider::Antigravity => Err(
            "Antigravity has no sign-in command: run `agy` in a terminal and finish the Google \
             sign-in it walks you through, then come back"
                .into(),
        ),
    }
}

/// Start the provider's login flow, streaming its output to `emit`; returns
/// once the child is spawned. stdin is piped (unlike `install.rs`) because
/// Claude's flow ends in a pasted code.
pub fn start<F>(provider: Provider, emit: F) -> Result<(), String>
where
    F: Fn(SignInLine) + Send + 'static,
{
    let args = login_args(provider)?;
    let bin = discover::binary(provider)?;

    // Spawn under the map's lock so two concurrent invokes cannot both pass
    // the check.
    let (stdout, stderr, stdin, cancelled, child) = {
        let mut r = running().lock().unwrap();
        if r.contains_key(&provider) {
            return Err(format!("{} is already signing in", provider.label()));
        }

        let mut child = Command::new(&bin)
            .args(args)
            .env_clear()
            .envs(discover::child_env())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot run {}: {e}", bin.display()))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdin = Arc::new(Mutex::new(child.stdin.take()));
        let cancelled = Arc::new(AtomicBool::new(false));
        let child = Arc::new(Mutex::new(child));

        r.insert(
            provider,
            Run {
                child: child.clone(),
                stdin: stdin.clone(),
                cancelled: cancelled.clone(),
            },
        );
        (stdout, stderr, stdin, cancelled, child)
    };

    std::thread::spawn(move || {
        let mut sent_url = false;
        super::install::drain_lines(stdout, stderr, |raw| {
            let line = strip_ansi(&raw);
            let url = if sent_url { None } else { extract_url(&line) };
            sent_url |= url.is_some();
            emit(SignInLine {
                provider,
                line: Some(line),
                url,
                done: false,
                ok: None,
                status: None,
            });
        });

        // Polled: a blocking `wait` under the child's mutex would deadlock
        // against `cancel`'s `kill`.
        let (ok, status) = loop {
            let reaped = child.lock().unwrap().try_wait();
            match reaped {
                Ok(Some(s)) if s.success() => break (true, "signed in".to_string()),
                Ok(Some(s)) => {
                    break if cancelled.load(Ordering::SeqCst) {
                        (false, "cancelled".to_string())
                    } else {
                        (false, super::install::exit_text(s))
                    }
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(25)),
                Err(e) => break (false, format!("could not be waited for: {e}")),
            }
        };

        drop(stdin.lock().unwrap().take());
        running().lock().unwrap().remove(&provider);
        emit(SignInLine {
            provider,
            line: None,
            url: None,
            done: true,
            ok: Some(ok),
            status: Some(status),
        });
    });

    Ok(())
}

/// Write a pasted authorization code to a waiting child's stdin (Claude only).
/// The newline commits it; without one the CLI waits forever.
pub fn submit_code(provider: Provider, code: &str) -> Result<(), String> {
    let stdin = {
        let r = running().lock().unwrap();
        let run = r
            .get(&provider)
            .ok_or_else(|| format!("{} is not signing in", provider.label()))?;
        run.stdin.clone()
    };
    let mut guard = stdin.lock().unwrap();
    let pipe = guard
        .as_mut()
        .ok_or_else(|| format!("{}'s sign-in is not reading input", provider.label()))?;
    writeln!(pipe, "{}", code.trim()).map_err(|e| format!("could not send the code: {e}"))?;
    pipe.flush()
        .map_err(|e| format!("could not send the code: {e}"))
}

/// Kill a run the student abandoned — the only way a flow ends early. The
/// supervisor removes the entry once the child is reaped, so a second `start`
/// cannot race a dying one.
pub fn cancel(provider: Provider) -> Result<(), String> {
    let r = running().lock().unwrap();
    let run = r
        .get(&provider)
        .ok_or_else(|| format!("{} is not signing in", provider.label()))?;
    run.cancelled.store(true, Ordering::SeqCst);
    let mut child = run.child.lock().unwrap();
    let _ = child.kill();
    Ok(())
}

// ── Reading the output ───────────────────────────────────────────────────────

/// Strip ANSI escape sequences — Codex colours its URL.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            // CSI: parameters, then a byte in @…~ ends it.
            Some('[') => {
                chars.next();
                for c in chars.by_ref() {
                    if matches!(c, '@'..='~') {
                        break;
                    }
                }
            }
            // OSC: ended by BEL, or by ESC \ — the ESC of which is eaten on
            // the next turn of the outer loop.
            Some(']') => {
                chars.next();
                for c in chars.by_ref() {
                    if c == '\u{7}' || c == '\u{1b}' {
                        break;
                    }
                }
            }
            // A two-character escape; drop both.
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// The first `https://` run in a line, minus trailing punctuation. Only
/// https: Codex prints its `http://localhost:1455` listener just before the
/// authorize URL.
fn extract_url(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let rest = &line[start..];
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let url = rest[..end].trim_end_matches(['.', ',', ')', '>']);
    (url.len() > "https://".len()).then(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_expired_session_message_is_an_auth_failure() {
        let msg = "Failed to authenticate: OAuth session expired and could not be refreshed";
        assert!(is_auth_failure(Provider::Claude, msg));
    }

    #[test]
    fn each_provider_knows_its_own_wording() {
        assert!(is_auth_failure(
            Provider::Claude,
            "Please run /login to authenticate"
        ));
        assert!(is_auth_failure(
            Provider::Claude,
            "Credentials are invalid, run `claude auth login`"
        ));
        assert!(is_auth_failure(
            Provider::Codex,
            "Not logged in. Run `codex login`."
        ));
        assert!(is_auth_failure(
            Provider::Codex,
            "Your ChatGPT account subscription has expired"
        ));
        assert!(is_auth_failure(
            Provider::Opencode,
            "AI_APICallError: incorrect API key provided"
        ));
        assert!(is_auth_failure(
            Provider::Opencode,
            "no credentials for provider anthropic"
        ));
        for p in [Provider::Claude, Provider::Codex, Provider::Opencode] {
            assert!(is_auth_failure(p, "OAuth token has expired"), "{p:?}");
            assert!(
                is_auth_failure(p, "authentication_error: invalid x-api-key"),
                "{p:?}"
            );
        }
    }

    #[test]
    fn ordinary_failures_are_not_auth_failures() {
        let innocent = [
            "error[E0308]: mismatched types",
            "No such file or directory (os error 2)",
            "rate limit reached, retry after 30s",
            "wrote 401 bytes to /tmp/oculus/out.log",
            "/var/log/401/report.txt: not found",
            "claude exited (code Some(1)) mid-turn",
            "turn failed: the model returned no content",
            "codex app-server exited (code Some(2))",
            "ENOENT: could not open agents/notes.md",
        ];
        for p in [Provider::Claude, Provider::Codex, Provider::Opencode] {
            for msg in innocent {
                assert!(!is_auth_failure(p, msg), "{p:?} wrongly flagged {msg:?}");
            }
        }
    }

    #[test]
    fn a_bare_401_is_not_enough() {
        assert!(!is_auth_failure(Provider::Claude, "server answered 401"));
        assert!(is_auth_failure(Provider::Claude, "401 Unauthorized"));
        assert!(is_auth_failure(
            Provider::Codex,
            "HTTP 401 while refreshing auth token"
        ));
    }

    /// Claude's block, verbatim; the prompt carries no trailing newline.
    #[test]
    fn claude_login_output_yields_its_authorize_url() {
        let block = "Opening browser to sign in…\n\
             If the browser didn't open, visit: https://claude.com/cai/oauth/authorize?code=true&client_id=abc123\n\
             Paste code here if prompted >";
        let urls: Vec<String> = block.lines().filter_map(extract_url).collect();
        assert_eq!(
            urls,
            vec!["https://claude.com/cai/oauth/authorize?code=true&client_id=abc123"]
        );
    }

    /// Codex's block, verbatim.
    #[test]
    fn codex_login_output_skips_the_loopback_line() {
        let block = "Starting local login server on http://localhost:1455.\n\
             If your browser did not open, navigate to this URL to authenticate:\n\
             \n\
             https://auth.openai.com/oauth/authorize?client_id=app_X&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback&scope=openid\n\
             \n\
             On a remote or headless machine? Use `codex login --device-auth` instead.";
        let urls: Vec<String> = block.lines().filter_map(extract_url).collect();
        assert_eq!(
            urls,
            vec!["https://auth.openai.com/oauth/authorize?client_id=app_X&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback&scope=openid"]
        );
    }

    #[test]
    fn a_url_is_trimmed_of_the_sentence_around_it() {
        assert_eq!(
            extract_url("visit https://example.com/a?b=1.").as_deref(),
            Some("https://example.com/a?b=1")
        );
        assert_eq!(
            extract_url("(see https://example.com/a)").as_deref(),
            Some("https://example.com/a")
        );
        assert_eq!(extract_url("nothing to open here"), None);
        assert_eq!(extract_url("http://localhost:1455/auth/callback"), None);
    }

    #[test]
    fn ansi_is_stripped_before_the_url_is_read() {
        let line = "\u{1b}[1mvisit:\u{1b}[0m \u{1b}[4mhttps://auth.openai.com/oauth/authorize?x=1\u{1b}[0m";
        let clean = strip_ansi(line);
        assert_eq!(clean, "visit: https://auth.openai.com/oauth/authorize?x=1");
        assert_eq!(
            extract_url(&clean).as_deref(),
            Some("https://auth.openai.com/oauth/authorize?x=1")
        );
    }

    #[test]
    fn claude_status_json_parses_both_answers() {
        let out = r#"{"loggedIn": false, "authMethod": "none", "apiProvider": "firstParty"}"#;
        assert_eq!(parse_claude_status(out), Some((false, None)));

        let out = r#"{"loggedIn": true, "authMethod": "claude.ai", "apiProvider": "firstParty",
                      "email": "someone@example.com", "subscriptionType": "max"}"#;
        assert_eq!(
            parse_claude_status(out),
            Some((true, Some("someone@example.com".to_string())))
        );

        let out = r#"{"loggedIn": true, "authMethod": "claude.ai"}"#;
        assert_eq!(
            parse_claude_status(out),
            Some((true, Some("Claude subscription".to_string())))
        );

        let out = r#"{"loggedIn":true,"authMethod":"console"}"#;
        assert_eq!(
            parse_claude_status(out),
            Some((true, Some("Anthropic Console".to_string())))
        );

        // Unreadable is "cannot say", not "signed out".
        assert_eq!(parse_claude_status("Unknown command: auth"), None);
        assert_eq!(parse_claude_status(r#"{"authMethod":"claude.ai"}"#), None);
    }

    #[test]
    fn codex_status_line_names_the_account() {
        assert_eq!(
            codex_account("Logged in using ChatGPT\n").as_deref(),
            Some("ChatGPT")
        );
        assert_eq!(
            codex_account("Logged in using an API key\n").as_deref(),
            Some("an API key")
        );
        assert_eq!(codex_account("\n\n").as_deref(), None);
    }

    #[test]
    fn opencode_is_routed_to_its_own_dialog() {
        let s = status(Provider::Opencode);
        assert_eq!(s.signed_in, None);
        assert_eq!(s.error, None);
        assert!(start(Provider::Opencode, |_| {}).is_err());
        assert!(login_args(Provider::Opencode).is_err());
    }

    #[test]
    fn code_and_cancel_need_a_run() {
        assert!(submit_code(Provider::Claude, "abc").is_err());
        assert!(cancel(Provider::Codex).is_err());
    }
}

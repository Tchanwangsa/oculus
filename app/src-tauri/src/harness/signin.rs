//! Signing in to a CLI agent, from the error row that says you are not.
//!
//! `install.rs` is the way through when the CLI is *missing*. This is the way
//! through when it is there and its credentials are not: an expired OAuth
//! session, a `claude auth logout` a student ran last month, a machine that
//! has never been signed in at all. Until this existed the whole answer was a
//! red row reading *"Failed to authenticate: OAuth session expired and could
//! not be refreshed"* — true, unactionable, and identical in shape to a row
//! about a syntax error in a file. A student's next move was to find a
//! terminal, remember which of three CLIs this thread was on, and guess the
//! subcommand.
//!
//! The module does two separable things, and they are worth keeping apart:
//!
//! - [`is_auth_failure`] **reads** a provider's own error text and says
//!   whether it is that provider saying it has no usable credentials. That is
//!   a classification over strings nobody controls, so the lists below are
//!   kept tight on purpose. A false positive is worse than a miss: it tells a
//!   student to re-authenticate over a failure that had nothing to do with
//!   their account, and the sign-in they then do will not fix it.
//! - [`start`] / [`submit_code`] / [`cancel`] **drive** the CLI's own login
//!   flow, which is the only login flow there is. Nothing here implements
//!   OAuth, holds a token, or writes a credential file. Each CLI owns its
//!   store and this module owns a subprocess.
//!
//! **The two flows are genuinely different shapes**, measured on this machine
//! rather than assumed:
//!
//! - `claude auth login` prints an authorize URL and then **blocks reading a
//!   pasted code from stdin**. Its last prompt — `Paste code here if prompted
//!   >` — carries no trailing newline, so a `lines()` loop never emits it
//!   until the child exits. That is expected and nothing here waits for it:
//!   the dialog's paste field is offered as soon as the URL arrives, and
//!   [`submit_code`] writes the answer to the child's stdin.
//! - `codex login` starts its **own loopback listener** on port 1455, prints
//!   the authorize URL, and finishes by itself when the browser redirect comes
//!   back. It never reads stdin. [`submit_code`] exists for one of the two
//!   providers and is simply never called for the other.
//!
//! **opencode is deliberately not here.** Its credentials are per *provider*,
//! not per CLI — one store holding an Anthropic key, a GitHub Copilot OAuth
//! token, and two hundred more — and this repo already has the whole surface
//! for it (`harness_opencode_providers`, `harness_opencode_set_key`, the
//! `harness_opencode_oauth_*` pair, and `OpencodeConnectDialog.tsx`), going
//! through `opencode serve`'s own HTTP API rather than a terminal. Driving
//! `opencode auth login` as a subprocess would be a second, worse door to the
//! same store: it is a TUI with arrow-key menus, and it could only write what
//! the existing path already writes. So [`status`] answers `signed_in: None`
//! for it — *not answerable from here*, which is a different fact from "no" —
//! and [`start`] returns an error naming the dialog that is the real answer.
//!
//! **Nothing here is cached, and that is the difference from
//! [`discover::health`](super::discover::health).** Health is asked by every
//! model picker in the app, three spawns a menu, so it is cached for the life
//! of the process and dropped only by Settings' *Recheck*. Sign-in state is
//! asked in two places — a settings row and an error row a student is looking
//! at *because something just failed* — and a cached "signed out" that
//! outlived the sign-in that fixed it would be the one wrong answer that
//! matters here. So [`status`] spawns the CLI every time it is asked, and
//! `discover::forget()` has nothing of this module's to forget. The binary
//! lookup underneath it is still cached; where the CLI *is* does not change
//! when its credentials do.
//!
//! The streaming shape is `install.rs`'s, for the same reasons: one event per
//! line on one `app.emit` channel, stdout and stderr drained by a thread each
//! into one channel (read one after the other, a child that fills the unread
//! pipe deadlocks), and a final `done` event that is always last. Two things
//! differ. The child is the **discovered binary run directly**, never
//! `$SHELL -lc` — there is no user-supplied string anywhere near a shell here,
//! and no profile PATH to need. And it gets [`discover::child_env`], whose
//! strip of `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` is exactly as load-bearing
//! for a login as it is for a turn: a sign-in that landed on a stray key in
//! the shell would defeat the point of signing in.
//!
//! **No deadline is imposed from above.** An OAuth page can sit open for as
//! long as a person takes to find their password, switch accounts, or answer a
//! second factor on a phone in another room. A timeout here could only abandon
//! a flow that was still going; [`cancel`] is what ends one, and a student
//! closing the dialog is what calls it.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
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

/// Whether the provider has credentials, asked of the provider.
///
/// Blocking — spawns the CLI. Callers wrap it in `spawn_blocking`.
///
/// Both probes are read-only and neither opens a browser: `claude auth status`
/// prints and exits, `codex login status` prints and exits. The two disagree
/// about how to say "no", which is why this is a match rather than a table —
/// Claude answers exit 0 either way and puts the answer in JSON; Codex answers
/// with its exit status and one line of prose.
pub fn status(provider: Provider) -> SignInStatus {
    if provider == Provider::Opencode {
        // Not "no": not a question this door can answer. See the module docs.
        return SignInStatus::unknown(provider);
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
        Provider::Opencode => unreachable!("returned above"),
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
            // A `claude` old enough not to know `auth status --json` prints
            // something else entirely. Saying so beats guessing "no".
            None => SignInStatus {
                error: Some(format!(
                    "`claude auth status --json` did not answer in JSON: {}",
                    stdout.trim()
                )),
                ..SignInStatus::unknown(provider)
            },
        },
        // Codex prints `Logged in using ChatGPT` on **stderr**, not stdout —
        // so the account is read from whichever stream actually spoke, or the
        // row says a bare "Signed in" forever. The exit code is the answer to
        // *whether*; only the wording of *which* comes from the line.
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
        Provider::Opencode => unreachable!("returned above"),
    }
}

/// `{"loggedIn": false, "authMethod": "none", …}` → (signed in, account).
///
/// Parsed from the first `{` rather than from byte zero: a CLI that one day
/// prints a deprecation notice above its JSON should not read as "not signed
/// in". `loggedIn` missing entirely is `None` — an answer this cannot read is
/// not an answer of "no".
fn parse_claude_status(stdout: &str) -> Option<(bool, Option<String>)> {
    let start = stdout.find('{')?;
    let v: serde_json::Value = serde_json::from_str(stdout[start..].trim()).ok()?;
    let signed_in = v.get("loggedIn")?.as_bool()?;
    let email = v.get("email").and_then(|e| e.as_str()).filter(|e| !e.is_empty());
    let method = v.get("authMethod").and_then(|m| m.as_str()).unwrap_or("");
    Some((signed_in, claude_account(signed_in, email, method)))
}

/// What Claude calls the account, in words a student would recognise.
///
/// The email first, because it is the one field that answers *which* account
/// rather than which door was used, and on a machine with a personal login and
/// a work one that is the whole question. The method is the fallback for a
/// payload that carries none — an API key has no address behind it.
///
/// The method names are the CLI's own, and the dot in `claude.ai` is load
/// bearing: the arm was once spelled `claudeai`, matched nothing, and fell
/// through to `other`, which put the raw `claude.ai` on screen as if it were
/// an account name.
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

/// `Logged in using ChatGPT` → `ChatGPT`. Anything else keeps the whole line,
/// which is still more useful than nothing on a row that only has to say
/// *which* account.
fn codex_account(stdout: &str) -> Option<String> {
    let line = stdout.lines().find(|l| !l.trim().is_empty())?.trim();
    let account = match line.to_lowercase().find("using ") {
        Some(i) => line[i + "using ".len()..].trim(),
        None => line,
    };
    (!account.is_empty()).then(|| account.to_string())
}

// ── Classifying a failure ────────────────────────────────────────────────────

/// Phrases that mean "no usable credentials" whichever CLI said them.
///
/// Every one of these is a whole clause rather than a word, which is the
/// discipline that keeps the list safe: `"auth"` alone would match a file
/// named `auth.rs` in a compile error, and `"expired"` alone would match a
/// cached download.
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

/// Claude Code's own vocabulary for it, including the two it tells a student
/// to run.
const CLAUDE: &[&str] = &[
    "failed to authenticate",
    "please run /login",
    "run /login",
    "claude auth login",
    "credentials are invalid",
];

/// Codex is the narrow one on purpose. `"chatgpt account"` is not a failure by
/// itself — it appears in perfectly happy status output — so it only counts
/// beside a word that makes the sentence a complaint.
const CODEX: &[&str] = &["codex login", "please sign in"];
const CODEX_ACCOUNT_TROUBLE: &[&str] = &[
    "expired",
    "not ",
    "no ",
    "missing",
    "invalid",
    "failed",
    "required",
    "sign in",
];

/// opencode's are the provider store's, not the CLI's: what a provider answers
/// when the key behind it is wrong or absent.
const OPENCODE: &[&str] = &[
    "opencode auth login",
    "no credentials",
    "provider is not connected",
    "incorrect api key",
    "invalid x-api-key",
];

/// Whether `msg` is the provider saying it has no usable credentials, rather
/// than any other kind of failure.
///
/// This is the difference between a red row a student can only read and a row
/// with a Sign in button on it, so the lists above are kept tight rather than
/// generous. `401` is the one number here and it never counts alone: a byte
/// count, a line number and a path can all carry those three digits, so it has
/// to arrive beside the word the status code is actually about.
pub fn is_auth_failure(provider: Provider, msg: &str) -> bool {
    let m = msg.to_lowercase();
    if SHARED.iter().any(|n| m.contains(n)) {
        return true;
    }
    let own = match provider {
        Provider::Claude => CLAUDE,
        Provider::Codex => CODEX,
        Provider::Opencode => OPENCODE,
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

/// Where a login's output reaches the webview, one event per line, the way
/// `install::INSTALL_EVENT` carries an install's. The dialog that asked for
/// the sign-in is the only listener and it filters by provider.
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

/// A login in flight. The child handle is kept because two commands have to
/// reach back into it after [`start`] has returned: [`submit_code`] writes to
/// its stdin, [`cancel`] kills it.
struct Run {
    child: Arc<Mutex<Child>>,
    /// Taken from the child once and held here, since Claude's flow writes to
    /// it minutes after the spawn. `None` only if the pipe could not be taken.
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    /// Set by [`cancel`], read by the supervisor, so a killed child is
    /// reported as abandoned rather than as "stopped by a signal" — the same
    /// exit, two different stories.
    cancelled: Arc<AtomicBool>,
}

/// One login at a time per provider. A second `codex login` would find port
/// 1455 taken and fail in a way nobody could read; a second `claude auth
/// login` would leave two children waiting on two stdins for one pasted code.
fn running() -> &'static Mutex<HashMap<Provider, Run>> {
    static R: OnceLock<Mutex<HashMap<Provider, Run>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

fn login_args(provider: Provider) -> Result<&'static [&'static str], String> {
    match provider {
        // `--claudeai` is the default and is left off for that reason: naming
        // it would pin this to the subscription flow, and a student on a
        // Console account signs in through the same subcommand.
        Provider::Claude => Ok(&["auth", "login"]),
        Provider::Codex => Ok(&["login"]),
        Provider::Opencode => Err(
            "opencode signs in per provider, in Settings → AI → Providers, not through a login \
             command"
                .into(),
        ),
    }
}

/// Start the provider's login flow, streaming its output to `emit`.
///
/// Returns once the child is spawned; everything after that happens on the
/// supervisor thread. One run per provider at a time.
///
/// **stdin is piped, not `/dev/null`.** That is the one place this differs
/// from `install.rs`, which nulls stdin precisely so that anything asking a
/// question fails instead of hanging. Here the question is the point: Claude's
/// flow *ends* in a pasted code, and a null stdin would make it fail at the
/// last step. Codex never reads the pipe, which costs nothing.
pub fn start<F>(provider: Provider, emit: F) -> Result<(), String>
where
    F: Fn(SignInLine) + Send + 'static,
{
    let args = login_args(provider)?;
    let bin = discover::binary(provider)?;

    // The spawn happens **under** the map's lock, not between a check and an
    // insert. Two invokes land on the blocking pool as two threads, and a
    // gap here would let both past the check: two `codex login` children
    // racing for port 1455, or two `claude auth login` children waiting on
    // two stdins for one single-use code. Nothing else locks this map while
    // a spawn is in flight, so holding it across one is free.
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
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let mut readers = Vec::new();
        for pipe in [
            stdout.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
            stderr.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let tx = tx.clone();
            readers.push(std::thread::spawn(move || {
                for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            }));
        }
        drop(tx);

        let mut sent_url = false;
        // Ends when both readers have hung up, which is after the child has
        // closed both pipes — so no output can arrive after the `done` below.
        for raw in rx {
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
        }
        for r in readers {
            let _ = r.join();
        }

        // Polled rather than waited under the lock: a blocking `wait` holding
        // the child's mutex would deadlock against the `kill` in `cancel`,
        // which is the one thing that could make it return.
        let (ok, status) = loop {
            let reaped = child.lock().unwrap().try_wait();
            match reaped {
                Ok(Some(s)) if s.success() => break (true, "signed in".to_string()),
                Ok(Some(s)) => {
                    break if cancelled.load(Ordering::SeqCst) {
                        (false, "cancelled".to_string())
                    } else {
                        (
                            false,
                            match s.code() {
                                Some(c) => format!("exited with status {c}"),
                                None => "stopped by a signal".to_string(),
                            },
                        )
                    }
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(25)),
                Err(e) => break (false, format!("could not be waited for: {e}")),
            }
        };

        // The stdin pipe goes with the run: holding it open past the child
        // would keep a dead write target reachable from `submit_code`.
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

/// Write a pasted authorization code to a waiting child's stdin. Claude's
/// flow ends here; Codex's never needs it.
///
/// The newline is what commits it: the CLI is reading a line, and a code
/// written without one sits in the pipe looking exactly like a hang.
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
    pipe.flush().map_err(|e| format!("could not send the code: {e}"))
}

/// Kill a run the student abandoned. Nothing above imposes a deadline — an
/// OAuth page can sit open for as long as a person takes — so this is the only
/// thing that ends a flow that is not going to finish.
///
/// The entry is left for the supervisor to remove: it is the thread that knows
/// the child is actually reaped, and removing it here would let a second
/// `start` spawn a child while the first was still dying on port 1455.
pub fn cancel(provider: Provider) -> Result<(), String> {
    let r = running().lock().unwrap();
    let run = r
        .get(&provider)
        .ok_or_else(|| format!("{} is not signing in", provider.label()))?;
    run.cancelled.store(true, Ordering::SeqCst);
    let mut child = run.child.lock().unwrap();
    // An already-exited child is not a failure to cancel: the supervisor is
    // simply a few milliseconds ahead.
    let _ = child.kill();
    Ok(())
}

// ── Reading the output ───────────────────────────────────────────────────────

/// Strip ANSI escape sequences. Codex colours its output, and a URL with a
/// reset sequence welded to its tail is not a URL — it is a link that opens a
/// 404 and a Copy button that pastes junk.
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

/// The first `https://` run in a line, to the end of whitespace.
///
/// **Only `https://`**, which is doing real work: Codex prints
/// `Starting local login server on http://localhost:1455.` an instant before
/// the authorize URL, and that loopback address is not somewhere to send a
/// student's browser. Claude's authorize URL is likewise the first `https://`
/// on its line, after `If the browser didn't open, visit: `.
///
/// The trailing trim is for the sentence the URL is embedded in: a full stop
/// or a closing bracket after it belongs to the prose, not to the address.
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

    /// The real thing, off the screenshot that started this: the row a student
    /// sees when Claude's OAuth session has lapsed has to become a sign-in
    /// card rather than red text.
    #[test]
    fn the_real_expired_session_message_is_an_auth_failure() {
        let msg = "Failed to authenticate: OAuth session expired and could not be refreshed";
        assert!(is_auth_failure(Provider::Claude, msg));
    }

    /// Each provider's own vocabulary, and the shared clauses under all three.
    #[test]
    fn each_provider_knows_its_own_wording() {
        assert!(is_auth_failure(Provider::Claude, "Please run /login to authenticate"));
        assert!(is_auth_failure(
            Provider::Claude,
            "Credentials are invalid, run `claude auth login`"
        ));
        assert!(is_auth_failure(Provider::Codex, "Not logged in. Run `codex login`."));
        assert!(is_auth_failure(
            Provider::Codex,
            "Your ChatGPT account subscription has expired"
        ));
        assert!(is_auth_failure(
            Provider::Opencode,
            "AI_APICallError: incorrect API key provided"
        ));
        assert!(is_auth_failure(Provider::Opencode, "no credentials for provider anthropic"));
        for p in [Provider::Claude, Provider::Codex, Provider::Opencode] {
            assert!(is_auth_failure(p, "OAuth token has expired"), "{p:?}");
            assert!(is_auth_failure(p, "authentication_error: invalid x-api-key"), "{p:?}");
        }
    }

    /// The half that matters more. An ordinary failure must never be dressed
    /// up as "sign in again" — a student who does sign in again finds the same
    /// error waiting, and now distrusts the card.
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

    /// `401` is never evidence on its own, and is evidence beside the word the
    /// status code is about.
    #[test]
    fn a_bare_401_is_not_enough() {
        assert!(!is_auth_failure(Provider::Claude, "server answered 401"));
        assert!(is_auth_failure(Provider::Claude, "401 Unauthorized"));
        assert!(is_auth_failure(Provider::Codex, "HTTP 401 while refreshing auth token"));
    }

    /// Claude's block, verbatim. The URL is the first `https://` on the third
    /// line, and the prompt that follows carries no newline of its own — which
    /// is why the run is not waiting for it.
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

    /// Codex's block, verbatim — including the loopback line above the real
    /// one, which is the whole reason only `https://` counts.
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

    /// Colour does not survive into the URL, or into the line the dialog
    /// shows.
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

    /// `claude auth status --json`, both ways round, as the CLI prints them.
    #[test]
    fn claude_status_json_parses_both_answers() {
        let out = r#"{"loggedIn": false, "authMethod": "none", "apiProvider": "firstParty"}"#;
        assert_eq!(parse_claude_status(out), Some((false, None)));

        // The shape 3.x actually prints, email and all.
        let out = r#"{"loggedIn": true, "authMethod": "claude.ai", "apiProvider": "firstParty",
                      "email": "someone@example.com", "subscriptionType": "max"}"#;
        assert_eq!(
            parse_claude_status(out),
            Some((true, Some("someone@example.com".to_string())))
        );

        // No address to show: the door is the next best answer, and the dotted
        // spelling is the CLI's.
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

        // Not JSON at all, and JSON that does not answer the question: both
        // are "cannot say", not "signed out".
        assert_eq!(parse_claude_status("Unknown command: auth"), None);
        assert_eq!(parse_claude_status(r#"{"authMethod":"claude.ai"}"#), None);
    }

    #[test]
    fn codex_status_line_names_the_account() {
        assert_eq!(codex_account("Logged in using ChatGPT\n").as_deref(), Some("ChatGPT"));
        assert_eq!(
            codex_account("Logged in using an API key\n").as_deref(),
            Some("an API key")
        );
        assert_eq!(codex_account("\n\n").as_deref(), None);
    }

    /// opencode is answered, not attempted: the status says "cannot say from
    /// here" and the flow refuses with a pointer to the door that works.
    #[test]
    fn opencode_is_routed_to_its_own_dialog() {
        let s = status(Provider::Opencode);
        assert_eq!(s.signed_in, None);
        assert_eq!(s.error, None);
        assert!(start(Provider::Opencode, |_| {}).is_err());
        assert!(login_args(Provider::Opencode).is_err());
    }

    /// Nothing to write to and nothing to kill when no run is in flight —
    /// both say so rather than panicking on an empty map.
    #[test]
    fn code_and_cancel_need_a_run() {
        assert!(submit_code(Provider::Claude, "abc").is_err());
        assert!(cancel(Provider::Codex).is_err());
    }
}

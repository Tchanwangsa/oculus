//! Driving a CLI's login flow as a subprocess, one at a time per provider.

use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;

use super::output::{extract_url, strip_ansi};
use crate::harness::cli::discover;
use crate::harness::cli::install::{drain_lines, exit_text};
use crate::harness::event::Provider;

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

pub(super) fn login_args(provider: Provider) -> Result<&'static [&'static str], String> {
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
/// once the child is spawned. stdin is piped (unlike `install`) because
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
        drain_lines(stdout, stderr, |raw| {
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
                        (false, exit_text(s))
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

//! Driving a CLI's login flow as a subprocess, one at a time per provider.

use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;

use super::agy;
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
    /// The authorize URL: once for Claude and Codex, once per round for
    /// Antigravity, whose every attempt has its own.
    pub url: Option<String>,
    /// Something the student must see while the run carries on — the
    /// sentence Antigravity refused a pasted code with.
    pub notice: Option<String>,
    /// The last event of a run.
    pub done: bool,
    pub ok: Option<bool>,
    pub status: Option<String>,
}

impl SignInLine {
    fn empty(provider: Provider) -> Self {
        SignInLine {
            provider,
            line: None,
            url: None,
            notice: None,
            done: false,
            ok: None,
            status: None,
        }
    }

    pub(super) fn line(provider: Provider, line: String, url: Option<String>) -> Self {
        SignInLine {
            line: Some(line),
            url,
            ..Self::empty(provider)
        }
    }

    pub(super) fn url(provider: Provider, url: String) -> Self {
        SignInLine {
            url: Some(url),
            ..Self::empty(provider)
        }
    }

    pub(super) fn notice(provider: Provider, notice: String) -> Self {
        SignInLine {
            notice: Some(notice),
            ..Self::empty(provider)
        }
    }

    pub(super) fn done(provider: Provider, ok: bool, status: String) -> Self {
        SignInLine {
            done: true,
            ok: Some(ok),
            status: Some(status),
            ..Self::empty(provider)
        }
    }
}

/// A login in flight: [`submit_code`] writes to it, [`cancel`] kills it.
pub(super) struct Run {
    pub(super) child: Arc<Mutex<Child>>,
    pub(super) input: Input,
    /// Set by [`cancel`], so a killed child reports "cancelled", not a signal.
    pub(super) cancelled: Arc<AtomicBool>,
}

/// Where a pasted code goes.
pub(super) enum Input {
    /// Claude's stdin; `None` only if the pipe could not be taken.
    Pipe(Arc<Mutex<Option<ChildStdin>>>),
    /// Antigravity's terminal.
    Terminal(Arc<agy::Terminal>),
}

/// One login at a time per provider: a second `codex login` would find port
/// 1455 taken, a second `claude auth login` would split one pasted code.
fn running() -> &'static Mutex<HashMap<Provider, Run>> {
    static R: OnceLock<Mutex<HashMap<Provider, Run>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Register the run `spawn` starts, under the map's lock so two concurrent
/// invokes cannot both pass the check; `T` is what the supervisor keeps.
pub(super) fn claim<T>(
    provider: Provider,
    spawn: impl FnOnce() -> Result<(Run, T), String>,
) -> Result<T, String> {
    let mut r = running().lock().unwrap();
    if r.contains_key(&provider) {
        return Err(format!("{} is already signing in", provider.label()));
    }
    let (run, keep) = spawn()?;
    r.insert(provider, run);
    Ok(keep)
}

/// Drop a finished run, so the next `start` may begin.
pub(super) fn release(provider: Provider) {
    running().lock().unwrap().remove(&provider);
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
        // Bare `agy`, in a terminal: see `agy::start`.
        Provider::Antigravity => Ok(&[]),
    }
}

/// Start the provider's login flow, streaming its output to `emit`; returns
/// once the child is spawned. stdin is piped (unlike `install`) because
/// Claude's flow ends in a pasted code; Antigravity's runs in a terminal.
pub fn start<F>(provider: Provider, emit: F) -> Result<(), String>
where
    F: Fn(SignInLine) + Send + 'static,
{
    let args = login_args(provider)?;
    if provider == Provider::Antigravity {
        return agy::start(emit);
    }
    let bin = discover::binary(provider)?;

    let (stdout, stderr, stdin, cancelled, child) = claim(provider, || {
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
        let run = Run {
            child: child.clone(),
            input: Input::Pipe(stdin.clone()),
            cancelled: cancelled.clone(),
        };
        Ok((run, (stdout, stderr, stdin, cancelled, child)))
    })?;

    std::thread::spawn(move || {
        let mut sent_url = false;
        drain_lines(stdout, stderr, |raw| {
            let line = strip_ansi(&raw);
            let url = if sent_url { None } else { extract_url(&line) };
            sent_url |= url.is_some();
            emit(SignInLine::line(provider, line, url));
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
        release(provider);
        emit(SignInLine::done(provider, ok, status));
    });

    Ok(())
}

/// Hand a pasted authorization code to a waiting login: Claude's stdin, or
/// Antigravity's code box. The newline commits it; without one the CLI
/// waits forever.
pub fn submit_code(provider: Provider, code: &str) -> Result<(), String> {
    let stdin = {
        let r = running().lock().unwrap();
        let run = r
            .get(&provider)
            .ok_or_else(|| format!("{} is not signing in", provider.label()))?;
        match &run.input {
            Input::Pipe(stdin) => stdin.clone(),
            Input::Terminal(t) => {
                let t = t.clone();
                drop(r);
                return t.submit(code);
            }
        }
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
    if let Input::Terminal(t) = &run.input {
        t.kill();
    }
    let mut child = run.child.lock().unwrap();
    let _ = child.kill();
    Ok(())
}

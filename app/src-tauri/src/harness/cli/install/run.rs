//! Running an install route in a login shell and streaming its output. The
//! runner is shared with `update` and `signin` through [`run_command`],
//! [`drain_lines`] and [`exit_text`].

use std::io::{BufRead, BufReader};
use std::process::{ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use super::routes::{command_for, Manager};
use crate::harness::event::Provider;

/// Where an install's output reaches the webview; the dialog filters by
/// provider.
pub const INSTALL_EVENT: &str = "harness-install";

/// What the webview gets, line by line, while an install runs.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallLine {
    pub provider: Provider,
    /// One line of the child's output, stdout and stderr both.
    pub line: Option<String>,
    /// The last event of a run, and the one the frontend rechecks on.
    pub done: bool,
    pub ok: Option<bool>,
    /// How it ended, for the line the dialog draws when it did not end well.
    pub status: Option<String>,
}

/// One install at a time per provider.
fn running() -> &'static Mutex<std::collections::HashSet<Provider>> {
    static R: OnceLock<Mutex<std::collections::HashSet<Provider>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Run a route, streaming its output to `emit` and closing with one `done`
/// line. Returns as soon as the child is spawned; everything after that
/// happens on the supervisor thread.
pub fn start<F>(provider: Provider, manager: Manager, emit: F) -> Result<(), String>
where
    F: Fn(InstallLine) + Send + 'static,
{
    let command = command_for(provider, manager).ok_or_else(|| {
        format!(
            "there is no {} route for {}",
            manager.label(),
            provider.label()
        )
    })?;

    {
        let mut r = running().lock().unwrap();
        if !r.insert(provider) {
            return Err(format!("{} is already installing", provider.label()));
        }
    }

    let finish = move |p: Provider| {
        running().lock().unwrap().remove(&p);
    };

    match run_command(command, "installed", move |line| {
        let done = line.done;
        emit(InstallLine {
            provider,
            line: line.line,
            done,
            ok: line.ok,
            status: line.status,
        });
        if done {
            finish(provider);
        }
    }) {
        Ok(()) => Ok(()),
        Err(e) => {
            running().lock().unwrap().remove(&provider);
            Err(e)
        }
    }
}

/// [`InstallLine`] without the provider — what the runner produces, so a
/// test can drive it with a harmless command.
#[derive(Clone, Debug)]
pub struct Line {
    pub line: Option<String>,
    pub done: bool,
    pub ok: Option<bool>,
    pub status: Option<String>,
}

/// Spawn `$SHELL -lc "<command>"` and stream it, `success` being the status
/// of a zero exit. stdin is `/dev/null`, so anything that asks a question (a
/// `sudo` password, a prompt) fails at once instead of hanging.
pub(in crate::harness::cli) fn run_command<F>(
    command: &str,
    success: &'static str,
    emit: F,
) -> Result<(), String>
where
    F: Fn(Line) + Send + 'static,
{
    // Guards the table against a future edit.
    if command.split_whitespace().any(|w| w == "sudo") {
        return Err("that command needs sudo, which this app cannot ask for".into());
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let mut child = Command::new(&shell)
        .args(["-lc", command])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run {shell}: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    std::thread::spawn(move || {
        drain_lines(stdout, stderr, |line| {
            emit(Line {
                line: Some(line),
                done: false,
                ok: None,
                status: None,
            })
        });
        let (ok, status) = match child.wait() {
            Ok(s) if s.success() => (true, success.to_string()),
            Ok(s) => (false, exit_text(s)),
            Err(e) => (false, format!("could not be waited for: {e}")),
        };
        emit(Line {
            line: None,
            done: true,
            ok: Some(ok),
            status: Some(status),
        });
    });
    Ok(())
}

/// Hand a child's stdout and stderr lines to `on_line` until both pipes
/// close — so nothing arrives after the caller's final `done`. One reader
/// thread per pipe: read in turn, a child filling the unread pipe deadlocks.
/// Order across the two streams is approximate.
pub(in crate::harness::cli) fn drain_lines(
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    mut on_line: impl FnMut(String),
) {
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
    for line in rx {
        on_line(line);
    }
    for r in readers {
        let _ = r.join();
    }
}

/// How a child that did not succeed ended.
pub(in crate::harness::cli) fn exit_text(s: ExitStatus) -> String {
    match s.code() {
        Some(c) => format!("exited with status {c}"),
        None => "stopped by a signal".to_string(),
    }
}

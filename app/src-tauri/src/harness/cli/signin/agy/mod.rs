//! Antigravity's sign-in: bare `agy` in a pseudo-terminal, answered by keys.
//!
//! `agy` has no login subcommand and refuses a pipe, so it runs on a PTY
//! (`super::pty`). Signed out, its TUI asks for a login method (Enter picks
//! Google OAuth), opens the authorize URL itself, and waits for the code
//! Google's callback page shows — there is no loopback listener. A refused
//! code ends on "Press any key to go back"; Enter returns to the menu, which
//! is answered again, so one run takes as many codes as the student pastes.
//! Signed in, agy moves on to a chat instead of exiting, so success is
//! confirmed by `agy models` — asked once a code is in and either agy's
//! success line shows or the screen goes quiet — and a yes ends the run and
//! kills agy. Killing only after that yes leaves agy time to store the token.

mod screen;
#[cfg(test)]
mod smoke;
#[cfg(test)]
mod tests;

use std::fs::File;
use std::io::{Read, Write};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use screen::{Phase, Screen, Step};

use super::flow::{claim, release, Input, Run, SignInLine};
use super::pty;
use crate::harness::cli::discover;
use crate::harness::cli::install::exit_text;
use crate::harness::event::Provider;

const PROVIDER: Provider = Provider::Antigravity;

/// Wide enough that the ~700-character authorize URL never wraps.
const COLS: u16 = 1000;
const ROWS: u16 = 50;

/// How long the screen stays still after a code before `agy models` is
/// asked; a spinner redraws while the exchange is in flight.
const QUIET: Duration = Duration::from_millis(1500);
/// The least time between two `agy models` checks.
const PROBE_GAP: Duration = Duration::from_secs(5);

/// The PTY and what has been read off it; shared by the supervisor and
/// [`super::submit_code`].
pub(super) struct Terminal {
    writer: Mutex<File>,
    screen: Mutex<Screen>,
    /// `agy`'s pid, which leads its own session and process group.
    pgid: u32,
}

impl Terminal {
    /// Type a pasted code into agy's code box and commit it.
    pub(super) fn submit(&self, code: &str) -> Result<(), String> {
        let code = code.trim();
        if code.is_empty() {
            return Err("paste the code first".into());
        }
        self.screen.lock().unwrap().code_sent(code)?;
        self.send(format!("{code}\r").as_bytes()).map_err(|e| {
            self.screen.lock().unwrap().code_lost();
            format!("could not send the code: {e}")
        })
    }

    fn send(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut w = self.writer.lock().unwrap();
        w.write_all(bytes)?;
        w.flush()
    }

    /// agy and its children (`open`) go together.
    pub(super) fn kill(&self) {
        pty::kill_group(self.pgid);
    }
}

/// How the supervisor's loop ended.
enum End {
    SignedIn,
    Cancelled,
    /// agy said why it cannot go on.
    Failed(String),
    /// agy exited, or its terminal closed.
    Exited,
}

pub(super) fn start<F>(emit: F) -> Result<(), String>
where
    F: Fn(SignInLine) + Send + 'static,
{
    let bin = discover::binary(PROVIDER)?;
    let mut env = discover::child_env();
    // The app is not started from a shell, so it has no TERM of its own.
    env.retain(|(k, _)| k != "TERM");
    env.push(("TERM".into(), "xterm-256color".into()));

    let (term, reader, child, cancelled) = claim(PROVIDER, || {
        let mut cmd = Command::new(&bin);
        cmd.env_clear().envs(env);
        let (child, master) = pty::spawn(cmd, COLS, ROWS)
            .map_err(|e| format!("cannot run {}: {e}", bin.display()))?;
        let pgid = child.id();
        let open = || -> std::io::Result<(File, File)> { Ok((master.try_clone()?, master)) };
        let (reader, writer) = match open() {
            Ok(rw) => rw,
            Err(e) => {
                pty::kill_group(pgid);
                return Err(format!("cannot read agy's terminal: {e}"));
            }
        };
        let term = Arc::new(Terminal {
            writer: Mutex::new(writer),
            screen: Mutex::new(Screen::new()),
            pgid,
        });
        let child = Arc::new(Mutex::new(child));
        let cancelled = Arc::new(AtomicBool::new(false));
        let run = Run {
            child: child.clone(),
            input: Input::Terminal(term.clone()),
            cancelled: cancelled.clone(),
        };
        Ok((run, (term, reader, child, cancelled)))
    })?;

    std::thread::spawn(move || {
        let mut reader = reader;
        let end = supervise(&term, &mut reader, &child, &cancelled, &emit);
        term.kill();
        // Polled, as in `flow::start`: `cancel` takes the same lock to kill.
        // A killed agy cannot finish exiting while its terminal holds unread
        // output (the last close waits for it to drain), so it is read and
        // dropped until agy is reaped.
        let reaped = loop {
            match child.lock().unwrap().try_wait() {
                Ok(Some(s)) => break Some(s),
                Ok(None) => drain(&mut reader),
                Err(_) => break None,
            }
        };
        let (ok, status) = match end {
            End::SignedIn => (true, "signed in".to_string()),
            End::Cancelled => (false, "cancelled".to_string()),
            End::Failed(sentence) => (false, sentence),
            End::Exited if cancelled.load(Ordering::SeqCst) => (false, "cancelled".to_string()),
            End::Exited if signed_in() => (true, "signed in".to_string()),
            End::Exited => (
                false,
                reaped.map_or("could not be waited for".to_string(), exit_text),
            ),
        };
        release(PROVIDER);
        emit(SignInLine::done(PROVIDER, ok, status));
    });
    Ok(())
}

/// Read the terminal until the sign-in ends one way or another, answering
/// the screens that want a key.
fn supervise<F>(
    term: &Terminal,
    reader: &mut File,
    child: &Mutex<std::process::Child>,
    cancelled: &AtomicBool,
    emit: &F,
) -> End
where
    F: Fn(SignInLine),
{
    let mut buf = vec![0u8; 16 * 1024];
    let mut last_output = Instant::now();
    let mut last_probe: Option<Instant> = None;
    let mut hinted = false;

    loop {
        if cancelled.load(Ordering::SeqCst) {
            return End::Cancelled;
        }
        match pty::readable(reader, 250) {
            Ok(true) => match reader.read(&mut buf) {
                // EOF, or EIO once agy has closed its side.
                Ok(0) | Err(_) => return End::Exited,
                Ok(n) => {
                    last_output = Instant::now();
                    let steps = term.screen.lock().unwrap().feed(&buf[..n]);
                    for step in steps {
                        match step {
                            Step::Line(line) => emit(SignInLine::line(PROVIDER, line, None)),
                            Step::Url(url) => emit(SignInLine::url(PROVIDER, url)),
                            Step::Key(key) => {
                                if let Err(e) = term.send(key) {
                                    elog!("[signin] could not answer agy's prompt: {e}");
                                }
                            }
                            Step::Refused(sentence) => emit(SignInLine::notice(
                                PROVIDER,
                                format!(
                                    "{sentence} A new sign-in page opens in your browser; \
                                     paste the code from that one."
                                ),
                            )),
                            Step::Fatal(sentence) => return End::Failed(sentence),
                            Step::SignedInHint => hinted = true,
                        }
                    }
                }
            },
            Ok(false) => {}
            Err(e) => {
                elog!("[signin] agy's terminal could not be polled: {e}");
                return End::Exited;
            }
        }

        if matches!(child.lock().unwrap().try_wait(), Ok(Some(_)) | Err(_)) {
            return End::Exited;
        }

        let exchanging = term.screen.lock().unwrap().phase() == Phase::Exchanging;
        let due = last_probe.is_none_or(|t| t.elapsed() >= PROBE_GAP);
        if exchanging && due && (hinted || last_output.elapsed() >= QUIET) {
            last_probe = Some(Instant::now());
            match super::status(PROVIDER).signed_in {
                Some(true) => return End::SignedIn,
                // agy said it worked and the check cannot say otherwise.
                None if hinted => return End::SignedIn,
                _ => {}
            }
        }
    }
}

/// Read and drop whatever the terminal holds, or wait a moment if it holds
/// nothing or has closed.
fn drain(reader: &mut File) {
    let mut buf = [0u8; 4096];
    match pty::readable(reader, 25) {
        Ok(true) => {
            if matches!(reader.read(&mut buf), Ok(0) | Err(_)) {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        Ok(false) => {}
        Err(_) => std::thread::sleep(Duration::from_millis(25)),
    }
}

/// `agy models` answers yes — the same check Settings makes.
fn signed_in() -> bool {
    super::status(PROVIDER).signed_in == Some(true)
}

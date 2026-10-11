//! Running ffmpeg: its stderr drained on a thread (ffmpeg blocks on a full
//! pipe) with the last lines kept for the error, and one-shot runs (concat,
//! mux) that the caller's cancel kills. No timeouts: a long video's mux
//! takes as long as it takes.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use super::error::{RenderError, Stop};

/// stderr lines an error quotes.
const TAIL_LINES: usize = 12;

/// The arguments every run starts with: errors only, no banner, no
/// keyboard interaction (stdin is the frame pipe or nothing).
pub(super) const QUIET: [&str; 5] = ["-hide_banner", "-nostdin", "-v", "error", "-y"];

/// The last `TAIL_LINES` lines of a child's stderr, read on their own thread.
pub(super) struct Tail(JoinHandle<VecDeque<String>>);

impl Tail {
    pub fn spawn(stderr: ChildStderr) -> Self {
        Self(std::thread::spawn(move || {
            let mut lines = VecDeque::with_capacity(TAIL_LINES);
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                if lines.len() == TAIL_LINES {
                    lines.pop_front();
                }
                lines.push_back(line);
            }
            lines
        }))
    }

    /// The lines, once the child has closed its stderr.
    pub fn join(self) -> String {
        let lines = self.0.join().unwrap_or_default();
        let text: Vec<&str> = lines
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        if text.is_empty() {
            "no output".to_owned()
        } else {
            text.join("\n")
        }
    }
}

/// `ffmpeg <args>`, `what` naming the step in the error, killed when
/// `cancel` is set.
pub(super) fn run(
    ffmpeg: &Path,
    args: &[&std::ffi::OsStr],
    what: &str,
    cancel: &AtomicBool,
) -> Result<(), Stop> {
    let mut child = spawn(
        Command::new(ffmpeg)
            .args(QUIET)
            .args(args)
            .stdin(Stdio::null()),
        what,
    )?;
    let tail = Tail::spawn(child.stderr.take().expect("piped stderr"));
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            child.kill().ok();
            child.wait().ok();
            tail.join();
            return Err(Stop::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                child.kill().ok();
                child.wait().ok();
                return Err(RenderError::Ffmpeg(format!("{what}: waiting for ffmpeg: {e}")).into());
            }
        }
    };
    let tail = tail.join();
    if status.success() {
        Ok(())
    } else {
        Err(failed(what, &status.to_string(), &tail).into())
    }
}

/// Start `command` with stdout discarded and stderr piped.
pub(super) fn spawn(command: &mut Command, what: &str) -> Result<Child, RenderError> {
    command
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| RenderError::Ffmpeg(format!("{what}: could not run ffmpeg: {e}")))
}

/// ffmpeg's non-zero exit, with its last stderr lines.
pub(super) fn failed(what: &str, status: &str, tail: &str) -> RenderError {
    RenderError::Ffmpeg(format!("{what}: ffmpeg {status}:\n{tail}"))
}

//! Plumbing shared by the bridges that own a CLI process: the process and its
//! stderr tail, JSON lines in and out, closing a turn the process died in,
//! and the spawn fields every per-thread CLI takes. Protocol stays in each
//! bridge.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::event::{HarnessEvent, Provider};
use super::{RawLog, Sink};

/// Enough stderr to explain an exit.
const TAIL_LINES: usize = 20;

/// What a per-thread CLI process (Claude, Antigravity) is started with.
pub struct ThreadSpawn {
    pub bin: PathBuf,
    /// The library's `agents/` folder: the cwd and the writable root.
    pub cwd: PathBuf,
    /// The library root, opened for reads with `--add-dir`.
    pub library: PathBuf,
    /// Resume this session rather than starting one.
    pub resume: Option<String>,
    pub model: Option<String>,
    /// Fixed for the process, so a level chosen mid-thread applies from the
    /// next resume.
    pub effort: Option<String>,
    pub env: Vec<(String, String)>,
    pub raw_log: Option<RawLog>,
}

/// A spawned CLI, killed on drop. `who` names it in error messages.
pub struct ChildProc {
    who: &'static str,
    child: Mutex<Child>,
    stdin: Option<Mutex<ChildStdin>>,
    alive: AtomicBool,
    tail: Arc<Mutex<VecDeque<String>>>,
}

impl ChildProc {
    /// Spawn `cmd` with stdout and stderr piped, and stdin too when `stdin`
    /// (else `/dev/null`). stderr feeds the tail; stdout is the caller's.
    pub fn spawn(who: &'static str, cmd: &mut Command, stdin: bool) -> Result<(Self, ChildStdout), String> {
        cmd.stdin(if stdin { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", Path::new(cmd.get_program()).display()))?;
        let pipe = match stdin {
            true => Some(Mutex::new(child.stdin.take().ok_or_else(|| format!("no stdin on {who} child"))?)),
            false => None,
        };
        let stdout = child.stdout.take().ok_or_else(|| format!("no stdout on {who} child"))?;
        let stderr = child.stderr.take().ok_or_else(|| format!("no stderr on {who} child"))?;

        let tail = Arc::new(Mutex::new(VecDeque::new()));
        {
            let tail = tail.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let mut t = tail.lock().unwrap();
                    if t.len() >= TAIL_LINES {
                        t.pop_front();
                    }
                    t.push_back(line);
                }
            });
        }
        let proc = ChildProc {
            who,
            child: Mutex::new(child),
            stdin: pipe,
            alive: AtomicBool::new(true),
            tail,
        };
        Ok((proc, stdout))
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Kill and reap. Marked dead first, so nothing sees a dying process as
    /// alive.
    pub fn kill(&self) -> Option<i32> {
        self.alive.store(false, Ordering::SeqCst);
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        child.wait().ok().and_then(|s| s.code())
    }

    /// After stdout's EOF: mark dead and reap.
    pub fn reap(&self) -> Option<i32> {
        self.alive.store(false, Ordering::SeqCst);
        self.child.lock().unwrap().wait().ok().and_then(|s| s.code())
    }

    /// One JSON value as one line on stdin.
    pub fn write_line(&self, v: &Value) -> Result<(), String> {
        let stdin = self.stdin.as_ref().ok_or_else(|| format!("no stdin on {} child", self.who))?;
        let mut stdin = stdin.lock().unwrap();
        let line = serde_json::to_string(v).map_err(|e| e.to_string())?;
        stdin
            .write_all(line.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("{} stdin: {e}", self.who))
    }

    /// `msg`, then the stderr tail after a colon when there is one.
    pub fn with_tail(&self, msg: String) -> String {
        let tail = self.tail.lock().unwrap().iter().map(String::as_str).collect::<Vec<_>>().join("\n");
        if tail.trim().is_empty() {
            msg
        } else {
            format!("{msg}:\n{tail}")
        }
    }

    /// A per-thread CLI's stdout has ended: reap it, fail the turn it died in
    /// (`mid_turn` is asked after the reap), then report the exit.
    pub fn finish(&self, sink: &Sink, provider: Provider, mid_turn: impl FnOnce() -> bool) {
        let code = self.reap();
        if mid_turn() {
            let msg = self.with_tail(format!("{} exited (code {code:?}) mid-turn", self.who));
            fail_turn(sink, provider, msg);
        }
        sink(HarnessEvent::Exited { code });
    }
}

impl Drop for ChildProc {
    fn drop(&mut self) {
        if let Ok(c) = self.child.get_mut() {
            let _ = c.kill();
        }
    }
}

/// Each stdout line that parses as JSON, raw-logged first; returns at EOF.
pub fn read_json_lines(stdout: ChildStdout, raw_log: Option<&RawLog>, mut on_value: impl FnMut(Value)) {
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if let Some(log) = raw_log {
            log.write(&line);
        }
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            on_value(v);
        }
    }
}

/// Close a turn that will never get its own end: the error, then `failed`.
pub fn fail_turn(sink: &Sink, provider: Provider, msg: String) {
    sink(HarnessEvent::error_for(provider, msg));
    sink(HarnessEvent::TurnFinished {
        status: "failed".into(),
    });
}

/// A string field, empty when missing or not a string.
pub fn str_of(v: &Value, key: &str) -> String {
    v.get(key).and_then(|s| s.as_str()).unwrap_or("").to_string()
}

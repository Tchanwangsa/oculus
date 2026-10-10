//! One `agy --print=` process per thread: spawn, send, interrupt.

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::harness::child::{self, ChildProc, ThreadSpawn};
use crate::harness::event::Provider;
use crate::harness::Sink;

use super::install;
use super::models::model_slug;
use super::translate::Translator;
use super::AntigravitySpawn;

pub struct AntigravitySession {
    pub(super) proc: ChildProc,
    pub(super) pending_brief: Mutex<Option<String>>,
    /// Set by [`Self::interrupt`], so the killed turn reads as interrupted.
    pub(super) interrupting: Arc<AtomicBool>,
    /// A turn is owed a `result`; a death meanwhile must still close it.
    pub(super) expecting: Arc<AtomicBool>,
}

/// `agy`'s argv after the binary.
pub(super) fn launch_args(base: &ThreadSpawn) -> Vec<String> {
    let mut args: Vec<String> = [
        // The `=` is load-bearing: see the module docs.
        "--print=",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        // A message starting with `/` is text, not a slash command.
        "--disable-slash-commands",
        // Shell commands only; file tools answer to the rules. This sandbox
        // lets `oculus` reach keyd's socket with no grant of its own. Never
        // add `--dangerously-skip-permissions`: it lets file tools write
        // outside the library (docs/harness.md).
        "--sandbox",
        // Claude's `acceptEdits`: workspace edits need no approval.
        "--mode",
        "accept-edits",
        "--add-dir",
    ]
    .map(String::from)
    .into();
    args.push(base.library.display().to_string());
    if let Some(m) = &base.model {
        args.extend(["--model".into(), model_slug(m, base.effort.as_deref())]);
    }
    if let Some(id) = &base.resume {
        args.extend(["--conversation".into(), id.clone()]);
    }
    args
}

impl AntigravitySession {
    pub fn spawn(cfg: AntigravitySpawn, sink: Sink) -> Result<Arc<Self>, String> {
        let base = cfg.base;
        // `agy` reads its rules once at start, so failing to write them fails
        // the spawn.
        install::install(&base.library, cfg.approved.clone())
            .map_err(|e| format!("Antigravity was not started: {e}"))?;
        let mut cmd = Command::new(&base.bin);
        cmd.args(launch_args(&base));
        cmd.current_dir(&base.cwd)
            .env_clear()
            .envs(base.env.iter().map(|(k, v)| (k, v)));
        let (proc, stdout) = ChildProc::spawn("agy", &mut cmd, true)?;

        let interrupting = Arc::new(AtomicBool::new(false));
        let expecting = Arc::new(AtomicBool::new(false));
        let session = Arc::new(AntigravitySession {
            proc,
            pending_brief: Mutex::new((!cfg.brief.trim().is_empty()).then(|| cfg.brief.clone())),
            interrupting: interrupting.clone(),
            expecting: expecting.clone(),
        });

        let reader_session = session.clone();
        let raw_log = base.raw_log;
        std::thread::spawn(move || {
            let mut state = Translator {
                interrupting,
                expecting: expecting.clone(),
                ..Default::default()
            };
            child::read_json_lines(stdout, raw_log.as_ref(), |v| {
                for ev in state.translate(&v) {
                    sink(ev);
                }
            });
            reader_session
                .proc
                .finish(&sink, Provider::Antigravity, || {
                    expecting.swap(false, Ordering::SeqCst) || state.turn_open
                });
        });

        Ok(session)
    }

    pub fn is_alive(&self) -> bool {
        self.proc.is_alive()
    }

    /// One user turn, with the brief ahead of the first message.
    pub fn send(&self, text: &str) -> Result<(), String> {
        let brief = self.pending_brief.lock().unwrap().take();
        let text = match brief {
            Some(b) => format!("{}\n\n---\n\n{text}", b.trim()),
            None => text.to_string(),
        };
        self.expecting.store(true, Ordering::SeqCst);
        self.proc.write_line(&serde_json::json!({
            "event": "user",
            "message": { "content": text },
        }))
    }

    /// Stop the current turn by killing the child (the protocol has no
    /// interrupt); the next message resumes the conversation by id.
    pub fn interrupt(&self) -> Result<(), String> {
        if !self.is_alive() {
            return Ok(());
        }
        self.interrupting.store(true, Ordering::SeqCst);
        self.kill();
        Ok(())
    }

    /// Refuses: agy 1.2.9 has no headless rewind (`/rewind` is not available
    /// in print mode), and the manager deletes rows on an `Ok`.
    pub fn rewind(&self, _anchor: &str) -> Result<(), String> {
        Err(
            "Antigravity cannot take a question back out of a conversation — \
             edit it in a new thread instead"
                .into(),
        )
    }

    pub fn kill(&self) {
        self.proc.kill();
    }
}

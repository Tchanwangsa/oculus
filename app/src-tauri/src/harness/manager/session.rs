//! Bringing a thread's session up and talking to it: send, rewind,
//! interrupt, close and the sweep on quit.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::harness::antigravity::{AntigravitySession, AntigravitySpawn};
use crate::harness::child::ThreadSpawn;
use crate::harness::claude::{ClaudeSession, ClaudeSpawn};
use crate::harness::codex::CodexThreadOpts;
use crate::harness::discover;
use crate::harness::opencode::{self, OpencodeSessionOpts};
use crate::harness::{Provider, Sink};

use super::handle::{Handle, Live};
use super::instructions::{instructions, thread_cwd, thread_sections};
use super::options::SendOptions;
use super::raw_log::RawLog;
use super::Harness;

impl Harness {
    /// Bring a thread's session up if it is not, then send. `resume` is the
    /// provider's session id from a previous process, if any.
    pub fn send(
        &self,
        thread_id: i64,
        provider: Provider,
        resume: Option<&str>,
        opts: &SendOptions,
        text: &str,
        sink: Sink,
    ) -> Result<(), String> {
        let mut live = self.live.lock().unwrap();
        self.ensure(&mut live, thread_id, provider, resume, opts, sink)?;
        live.get(&thread_id).ok_or("no session")?.handle.send(text)
    }

    /// Take a question and everything after it out of the provider's own
    /// session, so the agent's context matches the timeline. `anchor` is the
    /// provider's handle for that question, kept on its row; `last_seen` the
    /// newest question's, so Claude knows which later turns the timeline holds.
    /// A thread whose process has gone is resumed for this without starting a
    /// turn.
    pub fn rewind(
        &self,
        thread_id: i64,
        provider: Provider,
        resume: Option<&str>,
        opts: &SendOptions,
        anchor: &str,
        last_seen: Option<&str>,
        sink: Sink,
    ) -> Result<(), String> {
        // Rewound outside the lock: it waits on the CLI, and holding the map
        // would stall every other thread's next message.
        let handle = {
            let mut live = self.live.lock().unwrap();
            // Any live session will do — the reasoning level is irrelevant to
            // a control-channel instruction, so no `ensure` respawn.
            if !live.get(&thread_id).is_some_and(|l| l.is_alive()) {
                self.ensure(&mut live, thread_id, provider, resume, opts, sink)?;
            }
            live.get(&thread_id).ok_or("no session")?.handle.clone()
        };
        handle.rewind(anchor, last_seen)
    }

    /// Make sure this thread has a session that can be talked to, spawning or
    /// resuming one when it has none. A live session is reused only if it
    /// runs under the reasoning level asked for.
    fn ensure(
        &self,
        live: &mut HashMap<i64, Live>,
        thread_id: i64,
        provider: Provider,
        resume: Option<&str>,
        opts: &SendOptions,
        sink: Sink,
    ) -> Result<(), String> {
        if live
            .get(&thread_id)
            .is_some_and(|l| l.is_alive() && l.effort == opts.reasoning_effort)
        {
            return Ok(());
        }
        live.remove(&thread_id);

        let cwd = thread_cwd(&self.data_dir);
        std::fs::create_dir_all(&cwd)
            .map_err(|e| format!("cannot create {}: {e}", cwd.display()))?;
        let raw_log = RawLog::open(&self.data_dir, thread_id);
        let base = |cwd: PathBuf| -> Result<ThreadSpawn, String> {
            Ok(ThreadSpawn {
                bin: discover::binary(provider)?,
                cwd,
                library: self.data_dir.clone(),
                resume: resume.map(String::from),
                model: opts.model.clone(),
                effort: opts.reasoning_effort.clone(),
                env: discover::child_env(),
                raw_log,
            })
        };
        let handle = match provider {
            Provider::Claude => Handle::Claude(ClaudeSession::spawn(
                ClaudeSpawn {
                    base: base(cwd)?,
                    oculus: discover::oculus_cli(),
                    permission_mode: "acceptEdits".into(),
                    system_append: instructions(
                        &self.data_dir,
                        opts.scope.as_deref(),
                        opts.lecture.as_ref(),
                    ),
                    one_off: false,
                },
                sink,
            )?),
            Provider::Codex => {
                let server = self.codex_server()?;
                let topts = CodexThreadOpts {
                    cwd,
                    // Existing files only: Codex fails the whole turn on a
                    // writable root it cannot stat ("failed to inspect
                    // Seatbelt writable root"), e.g. an absent WAL sidecar.
                    writable_files: crate::library::paths::db_write_paths(&self.data_dir)
                        .into_iter()
                        .filter(|p| p.exists())
                        .collect(),
                    model: opts.model.clone(),
                    reasoning_effort: opts.reasoning_effort.clone(),
                    instructions: instructions(
                        &self.data_dir,
                        opts.scope.as_deref(),
                        opts.lecture.as_ref(),
                    ),
                    ephemeral: false,
                };
                let tid = match resume {
                    Some(id) => {
                        server.resume_thread(id, &topts, sink)?;
                        id.to_string()
                    }
                    None => server.start_thread(&topts, sink)?,
                };
                Handle::Codex(server, tid, Arc::new(topts))
            }
            Provider::Opencode => {
                let server = self.opencode_server()?;
                let sopts = OpencodeSessionOpts {
                    model: opts.model.clone(),
                    // Set only when the model declared levels.
                    variant: opts.reasoning_effort.clone(),
                    brief: thread_sections(opts.scope.as_deref(), opts.lecture.as_ref()),
                    agent: opencode::AGENT,
                };
                let ses = match resume {
                    Some(id) => {
                        server.attach_session(id, &sopts, sink)?;
                        id.to_string()
                    }
                    None => server.start_session(&sopts, sink)?,
                };
                Handle::Opencode(server, ses)
            }
            Provider::Antigravity => Handle::Antigravity(AntigravitySession::spawn(
                AntigravitySpawn {
                    base: base(cwd)?,
                    // `agy` reads the library-wide brief (`agents/AGENTS.md`)
                    // itself; only the per-thread half is sent.
                    brief: thread_sections(opts.scope.as_deref(), opts.lecture.as_ref()),
                    approved: opts.antigravity_rules.clone(),
                },
                sink,
            )?),
        };
        live.insert(
            thread_id,
            Live {
                handle,
                effort: opts.reasoning_effort.clone(),
            },
        );
        Ok(())
    }

    pub fn interrupt(&self, thread_id: i64) -> Result<(), String> {
        let live = self.live.lock().unwrap();
        match live.get(&thread_id) {
            Some(l) => l.handle.interrupt(),
            None => Ok(()),
        }
    }

    /// End the thread's process. The thread row stays; the next send
    /// resumes it by session id.
    pub fn close(&self, thread_id: i64) {
        if let Some(l) = self.live.lock().unwrap().remove(&thread_id) {
            l.handle.close(false);
        }
    }

    /// The threads with a live process of this provider's.
    pub fn live_threads(&self, provider: Provider) -> Vec<i64> {
        self.live
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, l)| l.handle.provider() == provider)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Everything, on quit.
    pub fn shutdown(&self) {
        self.drop_suggestions();
        let ids: Vec<i64> = self.live.lock().unwrap().keys().copied().collect();
        for id in ids {
            self.close(id);
        }
        if let Some(s) = self.codex.lock().unwrap().take() {
            s.kill();
        }
        if let Some(s) = self.opencode.lock().unwrap().take() {
            s.kill();
        }
    }
}

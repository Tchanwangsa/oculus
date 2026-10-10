//! Turns outside any thread: the namer, the job runners and `oculus agent`.
//! One session, one prompt, the reply collected, the session closed after.

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

use crate::harness::antigravity::{AntigravitySession, AntigravitySpawn};
use crate::harness::child::ThreadSpawn;
use crate::harness::claude::{ClaudeSession, ClaudeSpawn};
use crate::harness::codex::CodexThreadOpts;
use crate::harness::discover;
use crate::harness::jobs;
use crate::harness::opencode::{self, OpencodeSessionOpts};
use crate::harness::{HarnessEvent, Provider, Sink};

use super::handle::Handle;
use super::instructions::thread_cwd;
use super::naming::{clean_title, naming_prompt, NAMING_INSTRUCTIONS, NAMING_TIMEOUT_SECS};
use super::options::SendOptions;
use super::Harness;

impl Harness {
    /// Ask the provider to name a thread from its first exchange, on the
    /// agent and model the `threadNaming` job is configured with (`jobs.rs`).
    /// It runs in a throwaway session of its own, so the question never
    /// reaches the thread's timeline or context.
    pub fn name_thread(
        &self,
        sel: &jobs::JobSelection,
        first_message: &str,
        reply: &str,
    ) -> Result<String, String> {
        let turn = self.one_off(sel, NAMING_INSTRUCTIONS, opencode::NAMING_AGENT)?;
        let timeout = Some(std::time::Duration::from_secs(NAMING_TIMEOUT_SECS));
        let answer = turn
            .handle
            .send(&naming_prompt(first_message, reply))
            .map(|()| turn.wait(timeout, "naming the thread"));
        turn.close();
        let answer = answer?;
        let text = answer.message_or_streamed();
        match (clean_title(text), &answer.failed) {
            (Some(t), _) => Ok(t),
            (None, Some(e)) => Err(e.clone()),
            (None, None) => Err(format!("no usable name in the reply: {text:?}")),
        }
    }

    /// One tool-less turn outside any thread ([`Harness::one_off`]): `prompt`
    /// in, the reply's text out, the session closed after. No timeout: a job's
    /// stale `running` is swept at startup instead.
    pub fn one_turn(
        &self,
        sel: &jobs::JobSelection,
        instructions: &str,
        agent: &'static str,
        prompt: &str,
    ) -> Result<String, String> {
        let turn = self.one_off(sel, instructions, agent)?;
        let answer = turn
            .handle
            .send(prompt)
            .map(|()| turn.wait(None, "answering"));
        turn.close();
        let answer = answer?;
        let text = answer.message_or_streamed();
        match &answer.failed {
            Some(e) if text.trim().is_empty() => Err(e.clone()),
            None if text.trim().is_empty() => Err("the agent replied with nothing".to_string()),
            _ => Ok(text.to_string()),
        }
    }

    /// A tool-less session outside any thread, on `sel`'s agent, model and
    /// level, not yet prompted. `instructions` is its brief — appended to
    /// Claude's system prompt, Codex's developer instructions, ahead of agy's
    /// first message — except on opencode, whose `agent` carries it as its
    /// prompt. Nothing is persisted or raw-logged.
    pub(in crate::harness) fn one_off(
        &self,
        sel: &jobs::JobSelection,
        instructions: &str,
        agent: &'static str,
    ) -> Result<OneOff, String> {
        let provider = sel.provider;
        let (tx, rx) = mpsc::channel::<HarnessEvent>();
        let sink: Sink = Arc::new(move |ev| {
            let _ = tx.send(ev);
        });
        let cwd = thread_cwd(&self.data_dir);
        std::fs::create_dir_all(&cwd)
            .map_err(|e| format!("cannot create {}: {e}", cwd.display()))?;
        let base = |cwd: PathBuf| -> Result<ThreadSpawn, String> {
            Ok(ThreadSpawn {
                bin: discover::binary(provider)?,
                cwd,
                library: self.data_dir.clone(),
                resume: None,
                model: Some(sel.model.clone()),
                effort: sel.reasoning_effort.clone(),
                env: discover::child_env(),
                raw_log: None,
            })
        };

        let handle = match provider {
            Provider::Claude => Handle::Claude(ClaudeSession::spawn(
                ClaudeSpawn {
                    base: base(cwd)?,
                    oculus: discover::oculus_cli(),
                    // `default` auto-allows no tool; with prompts routed to
                    // `none` a stray call is refused rather than hanging.
                    permission_mode: "default".into(),
                    system_append: instructions.to_string(),
                    one_off: true,
                },
                sink,
            )?),
            Provider::Codex => {
                let server = self.codex_server()?;
                let opts = CodexThreadOpts {
                    cwd,
                    writable_files: Vec::new(),
                    model: Some(sel.model.clone()),
                    reasoning_effort: sel.reasoning_effort.clone(),
                    instructions: instructions.to_string(),
                    ephemeral: true,
                };
                let tid = server.start_thread(&opts, sink)?;
                Handle::Codex(server, tid, Arc::new(opts))
            }
            Provider::Opencode => {
                let server = self.opencode_server()?;
                // opencode has no per-session instructions, so the brief is
                // the hidden agent's prompt (`opencode::write_config`).
                let sopts = OpencodeSessionOpts {
                    model: Some(sel.model.clone()),
                    variant: sel.reasoning_effort.clone(),
                    brief: String::new(),
                    agent,
                };
                let ses = server.start_session(&sopts, sink)?;
                Handle::Opencode(server, ses)
            }
            Provider::Antigravity => Handle::Antigravity(AntigravitySession::spawn(
                AntigravitySpawn {
                    base: base(cwd)?,
                    brief: instructions.to_string(),
                    // No database here: keep the approvals last written.
                    approved: None,
                },
                sink,
            )?),
        };
        Ok(OneOff { handle, rx })
    }
}

/// A session [`Harness::one_off`] opened, and the events it answers on.
pub(in crate::harness) struct OneOff {
    pub(in crate::harness) handle: Handle,
    rx: mpsc::Receiver<HarnessEvent>,
}

/// What a one-off turn said. `streamed` is the deltas, which keep the leading
/// whitespace Claude's committed message trims off.
pub(in crate::harness) struct OneOffReply {
    pub(in crate::harness) message: String,
    pub(in crate::harness) streamed: String,
    pub(in crate::harness) failed: Option<String>,
}

impl OneOffReply {
    pub(in crate::harness) fn message_or_streamed(&self) -> &str {
        if self.message.trim().is_empty() {
            &self.streamed
        } else {
            &self.message
        }
    }
}

impl OneOff {
    /// Wait out the turn a prompt started.
    pub(in crate::harness) fn wait(
        &self,
        timeout: Option<std::time::Duration>,
        doing: &str,
    ) -> OneOffReply {
        let (mut message, mut streamed) = (String::new(), String::new());
        let failed = drain_turn(&self.rx, timeout, doing, |ev| match ev {
            HarnessEvent::AssistantMessage { text } => message.push_str(text),
            HarnessEvent::AssistantDelta { text } => streamed.push_str(text),
            _ => {}
        });
        OneOffReply {
            message,
            streamed,
            failed,
        }
    }

    /// Deleted rather than detached: a session left on the opencode server
    /// would sit in the student's own session list.
    pub(in crate::harness) fn close(&self) {
        self.handle.close(true);
    }
}

/// One prompt, one turn, events to `on_event`, then the process is gone.
/// What `oculus agent` runs; also the smallest end-to-end test of a bridge.
pub fn run_once(
    data_dir: &Path,
    provider: Provider,
    opts: &SendOptions,
    prompt: &str,
    on_event: impl Fn(&HarnessEvent) + Send + Sync + 'static,
) -> Result<(), String> {
    let harness = Harness::new(data_dir.to_path_buf());
    let (tx, rx) = mpsc::channel::<HarnessEvent>();
    let sink: Sink = Arc::new(move |ev| {
        let _ = tx.send(ev);
    });
    // Thread id 0 in the log dir: a headless run is not a thread.
    harness.send(0, provider, None, opts, prompt, sink)?;
    let failed = drain_turn(&rx, None, "finishing", &on_event);
    harness.shutdown();
    match failed {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Wait out one turn on `rx`, handing every event to `on_event`, and answer
/// its failure if it had one. With a `timeout`, that long a silence fails it.
fn drain_turn(
    rx: &mpsc::Receiver<HarnessEvent>,
    timeout: Option<std::time::Duration>,
    doing: &str,
    mut on_event: impl FnMut(&HarnessEvent),
) -> Option<String> {
    let mut failed: Option<String> = None;
    loop {
        let ev = match timeout {
            Some(t) => match rx.recv_timeout(t) {
                Ok(ev) => ev,
                Err(_) => {
                    failed.get_or_insert_with(|| format!("timed out {doing}"));
                    break;
                }
            },
            None => match rx.recv() {
                Ok(ev) => ev,
                Err(_) => break,
            },
        };
        on_event(&ev);
        match ev {
            HarnessEvent::Error { message, .. } => failed = Some(message),
            HarnessEvent::TurnFinished { .. } => break,
            HarnessEvent::Exited { code } => {
                failed.get_or_insert(format!("provider exited (code {code:?}) before {doing}"));
                break;
            }
            _ => {}
        }
    }
    failed
}

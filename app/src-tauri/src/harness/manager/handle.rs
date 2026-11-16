use std::sync::Arc;

use crate::harness::antigravity::acp::AcpSession;
use crate::harness::antigravity::AntigravitySession;
use crate::harness::claude::ClaudeSession;
use crate::harness::codex::{CodexServer, CodexThreadOpts};
use crate::harness::event::{unbound, Bound};
use crate::harness::opencode::OpencodeServer;
use crate::harness::Provider;

use super::sinks::{Generation, Sinks};
use super::transition::Selection;
use crate::harness::requests::PermissionMode;

/// Where a rewind goes back to: `anchor` is the provider's handle for the
/// question to take out, kept on its row; `last_seen` the newest question's,
/// so Claude knows which later turns the timeline holds.
#[derive(Clone, Copy)]
pub struct RewindTo<'a> {
    pub anchor: &'a str,
    pub last_seen: Option<&'a str>,
}

/// One provider session: a process per thread (Claude, Antigravity) or a
/// thread/session id on a shared server (Codex, opencode). Cheap to clone,
/// so it can be lifted out of the live map and talked to without the lock.
#[derive(Clone)]
pub(in crate::harness) enum Handle {
    Claude(Arc<ClaudeSession>),
    Codex(Arc<CodexServer>, String, Arc<CodexThreadOpts>),
    Opencode(Arc<OpencodeServer>, String),
    Antigravity(Arc<AntigravitySession>),
    /// Antigravity over ACP (`agy acp`), one process per thread.
    AntigravityAcp(Arc<AcpSession>),
}

impl Handle {
    pub(super) fn provider(&self) -> Provider {
        match self {
            Handle::Claude(_) => Provider::Claude,
            Handle::Codex(..) => Provider::Codex,
            Handle::Opencode(..) => Provider::Opencode,
            Handle::Antigravity(_) | Handle::AntigravityAcp(_) => Provider::Antigravity,
        }
    }

    pub(in crate::harness) fn is_alive(&self) -> bool {
        match self {
            Handle::Claude(s) => s.is_alive(),
            Handle::Codex(server, tid, _) => server.is_alive() && server.has_thread(tid),
            Handle::Opencode(server, ses) => server.is_alive() && server.has_session(ses),
            Handle::Antigravity(s) => s.is_alive(),
            Handle::AntigravityAcp(s) => s.is_alive(),
        }
    }

    /// The session has work running in the background that ends with it:
    /// Claude's backgrounded `Agent` and Bash. No other bridge has any.
    pub(super) fn has_background_work(&self) -> bool {
        match self {
            Handle::Claude(s) => s.has_background_tasks(),
            _ => false,
        }
    }

    /// The CLI is waiting on a callback nobody has answered.
    pub(super) fn has_pending_request(&self) -> bool {
        match self {
            Handle::Codex(server, tid, _) => server.has_pending_request(tid),
            Handle::Claude(s) => s.has_pending_request(),
            Handle::AntigravityAcp(s) => s.has_pending_request(),
            Handle::Opencode(..) | Handle::Antigravity(_) => false,
        }
    }

    /// The same session, sending its later turns under `mode`. Only Codex
    /// reads one, on each `turn/start`. Claude's is fixed at spawn (its
    /// settings say whether a command may leave the sandbox), so a change
    /// restarts it (`mode_switch_in_session` is false) and this keeps it as
    /// it is; the others ask no one and record it.
    pub(super) fn with_mode(&self, mode: PermissionMode) -> Handle {
        match self {
            Handle::Codex(server, tid, opts) => {
                let opts = CodexThreadOpts {
                    mode,
                    ..(**opts).clone()
                };
                Handle::Codex(server.clone(), tid.clone(), Arc::new(opts))
            }
            Handle::Claude(_)
            | Handle::Opencode(..)
            | Handle::Antigravity(_)
            | Handle::AntigravityAcp(_) => self.clone(),
        }
    }

    /// The same session, sending its later turns to `model`. Only for a
    /// bridge whose `model_switch_in_session` is true; the others keep the
    /// model they were spawned with.
    pub(super) fn with_model(&self, model: Option<String>) -> Handle {
        match self {
            Handle::Codex(server, tid, opts) => {
                let opts = CodexThreadOpts {
                    model,
                    ..(**opts).clone()
                };
                Handle::Codex(server.clone(), tid.clone(), Arc::new(opts))
            }
            Handle::Opencode(server, ses) => {
                server.set_model(ses, model);
                self.clone()
            }
            Handle::AntigravityAcp(s) => {
                s.want_model(model);
                self.clone()
            }
            Handle::Claude(_) | Handle::Antigravity(_) => self.clone(),
        }
    }

    /// A send nothing tracks: a one-off turn.
    pub(in crate::harness) fn send(&self, text: &str) -> Result<(), String> {
        self.send_turn(text, &unbound())
    }

    /// Send, telling `bound` which turn of the session the message was
    /// accepted as, before any event of that turn.
    pub(in crate::harness) fn send_turn(&self, text: &str, bound: &Bound) -> Result<(), String> {
        match self {
            Handle::Claude(s) => s.send(text, bound),
            Handle::Codex(server, tid, opts) => server.start_turn(tid, text, opts, bound),
            Handle::Opencode(server, ses) => server.prompt(ses, text, bound),
            Handle::Antigravity(s) => s.send(text, bound),
            Handle::AntigravityAcp(s) => s.send(text, bound),
        }
    }

    /// Antigravity refuses: its protocol has no way back to an earlier message.
    pub(super) fn rewind(&self, to: RewindTo) -> Result<(), String> {
        let RewindTo { anchor, last_seen } = to;
        match self {
            Handle::Claude(s) => s.rewind(anchor, last_seen),
            Handle::Codex(server, tid, _) => server.revert(tid, anchor),
            Handle::Opencode(server, ses) => server.revert(ses, anchor),
            Handle::Antigravity(s) => s.rewind(anchor),
            Handle::AntigravityAcp(s) => s.rewind(anchor),
        }
    }

    pub(super) fn interrupt(&self) -> Result<(), String> {
        match self {
            Handle::Claude(s) => s.interrupt(),
            Handle::Codex(server, tid, _) => server.interrupt(tid),
            Handle::Opencode(server, ses) => server.interrupt(ses),
            Handle::Antigravity(s) => s.interrupt(),
            Handle::AntigravityAcp(s) => s.interrupt(),
        }
    }

    /// Stop a one-off turn as fast as the provider allows: a process per turn
    /// is killed (its reader closes the turn), a server's turn interrupted.
    pub(in crate::harness) fn cancel(&self) -> Result<(), String> {
        match self {
            Handle::Claude(s) => {
                s.kill();
                Ok(())
            }
            Handle::Antigravity(s) => {
                s.kill();
                Ok(())
            }
            Handle::AntigravityAcp(s) => {
                s.kill();
                Ok(())
            }
            Handle::Codex(..) | Handle::Opencode(..) => self.interrupt(),
        }
    }

    /// End this session; a shared server stays up. With `delete`, an opencode
    /// session is removed from the server rather than detached.
    pub(super) fn close(&self, delete: bool) {
        match self {
            Handle::Claude(s) => s.kill(),
            Handle::Codex(server, tid, _) => server.detach(tid),
            Handle::Opencode(server, ses) if delete => server.delete_session(ses),
            Handle::Opencode(server, ses) => server.detach(ses),
            Handle::Antigravity(s) => s.kill(),
            Handle::AntigravityAcp(s) => s.kill(),
        }
    }
}

/// A thread's live session and what it was started under: the provider,
/// the model and the reasoning level (`transition.rs` reads them to decide
/// what the next send does to it).
pub(super) struct Live {
    pub(super) handle: Handle,
    pub(super) selection: Selection,
    /// The permission mode its turns are sent under.
    pub(super) mode: PermissionMode,
    /// Minted when this session was spawned or attached; its sink carries it.
    pub(super) generation: Generation,
    /// Where this session reports.
    pub(super) sinks: Arc<dyn Sinks>,
}

impl Live {
    pub(super) fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}

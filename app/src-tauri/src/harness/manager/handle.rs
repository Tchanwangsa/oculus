use std::sync::Arc;

use crate::harness::antigravity::AntigravitySession;
use crate::harness::claude::ClaudeSession;
use crate::harness::codex::{CodexServer, CodexThreadOpts};
use crate::harness::opencode::OpencodeServer;
use crate::harness::Provider;

/// One provider session: a process per thread (Claude, Antigravity) or a
/// thread/session id on a shared server (Codex, opencode). Cheap to clone,
/// so it can be lifted out of the live map and talked to without the lock.
#[derive(Clone)]
pub(in crate::harness) enum Handle {
    Claude(Arc<ClaudeSession>),
    Codex(Arc<CodexServer>, String, Arc<CodexThreadOpts>),
    Opencode(Arc<OpencodeServer>, String),
    Antigravity(Arc<AntigravitySession>),
}

impl Handle {
    pub(super) fn provider(&self) -> Provider {
        match self {
            Handle::Claude(_) => Provider::Claude,
            Handle::Codex(..) => Provider::Codex,
            Handle::Opencode(..) => Provider::Opencode,
            Handle::Antigravity(_) => Provider::Antigravity,
        }
    }

    pub(in crate::harness) fn is_alive(&self) -> bool {
        match self {
            Handle::Claude(s) => s.is_alive(),
            Handle::Codex(server, tid, _) => server.is_alive() && server.has_thread(tid),
            Handle::Opencode(server, ses) => server.is_alive() && server.has_session(ses),
            Handle::Antigravity(s) => s.is_alive(),
        }
    }

    pub(in crate::harness) fn send(&self, text: &str) -> Result<(), String> {
        match self {
            Handle::Claude(s) => s.send(text),
            Handle::Codex(server, tid, opts) => server.start_turn(tid, text, opts),
            Handle::Opencode(server, ses) => server.prompt(ses, text),
            Handle::Antigravity(s) => s.send(text),
        }
    }

    /// Antigravity refuses: its protocol has no way back to an earlier message.
    pub(super) fn rewind(&self, anchor: &str, last_seen: Option<&str>) -> Result<(), String> {
        match self {
            Handle::Claude(s) => s.rewind(anchor, last_seen),
            Handle::Codex(server, tid, _) => server.revert(tid, anchor),
            Handle::Opencode(server, ses) => server.revert(ses, anchor),
            Handle::Antigravity(s) => s.rewind(anchor),
        }
    }

    pub(super) fn interrupt(&self) -> Result<(), String> {
        match self {
            Handle::Claude(s) => s.interrupt(),
            Handle::Codex(server, tid, _) => server.interrupt(tid),
            Handle::Opencode(server, ses) => server.interrupt(ses),
            Handle::Antigravity(s) => s.interrupt(),
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
        }
    }
}

/// A thread's live session and the reasoning level it was started under.
/// Every CLI binds the level at session start, so a different one respawns.
pub(super) struct Live {
    pub(super) handle: Handle,
    pub(super) effort: Option<String>,
}

impl Live {
    pub(super) fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}

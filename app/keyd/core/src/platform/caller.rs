use std::path::PathBuf;

use super::Conn;

/// Which part of Oculus a caller is. Ops check this, never a path or a
/// signing identifier, so how an adapter tells them apart stays its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Role {
    App,
    Cli,
    #[default]
    Unknown,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::App => "app",
            Role::Cli => "cli",
            Role::Unknown => "unknown",
        }
    }
}

/// What keyd learned about a peer. Each lookup that failed leaves its field
/// empty and says why in `problems`.
#[derive(Debug, Default)]
pub struct Caller {
    pub uid: Option<u32>,
    pub pid: Option<u32>,
    pub path: Option<PathBuf>,
    /// The code-signing identifier, for the log.
    pub identifier: Option<String>,
    /// The running code is intact, as far as the OS can tell.
    pub valid: bool,
    pub role: Role,
    pub problems: Vec<String>,
}

impl Caller {
    /// For the log: the role, then the identifier, the path or the pid.
    pub fn label(&self) -> String {
        let path = self.path.as_ref().map(|p| p.display().to_string());
        let who = match (&self.identifier, path) {
            (Some(id), Some(p)) => format!("{id} ({p})"),
            (Some(id), None) => id.clone(),
            (None, Some(p)) => p,
            (None, None) => format!(
                "pid {}",
                self.pid.map_or("?".to_string(), |p| p.to_string())
            ),
        };
        format!("{} {who}", self.role.as_str())
    }
}

/// Whom keyd serves. Every policy requires keyd's own user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Same user is enough. Dev builds only: they have no install to check
    /// callers against.
    SameUser,
    /// Same user, and an executable that belongs to keyd's own Oculus
    /// install, which must verify as intact.
    Install,
}

pub trait PeerCheck: Send + Sync {
    /// Who is on the other end of `conn`, and in which role.
    fn inspect(&self, conn: &Conn) -> Caller;
    /// `Ok` to serve `caller`, or why not.
    fn admit(&self, caller: &Caller) -> Result<(), String>;
}

#[cfg(feature = "server")]
pub fn peer_check(policy: Policy) -> Box<dyn PeerCheck> {
    super::imp::peer_check(policy)
}

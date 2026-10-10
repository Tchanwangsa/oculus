//! The adapter contract: everything keyd and its clients need from an OS,
//! and the only place OS calls are made. The adapter is picked by target;
//! each delivers the same contract and never varies it.
//!
//! 1. Endpoint: `Listener::bind`, `accept_any` and `connect` over a `Conn`
//!    only this user can reach; `connect` is `Absent` when nothing is there.
//! 2. Activation: `activated()`, the listeners the OS made when the first
//!    connect started keyd, outside the caller's sandbox.
//! 3. PeerCheck: `inspect` (same user, which executable, its `Role`) and
//!    `admit`, both before a byte of the request is read.
//! 4, 5. `master_key()` and `legacy_items()`: the OS secret store.
//! 6. `registrar()`: installs, reports and removes what starts keyd, and
//!    retires a registration an earlier Oculus made.
//! 7. Paths are `crate::paths`; 8. the build step is the adapter's part of
//!    `app/scripts/build-keyd.mjs`. `files` holds the file helpers.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(not(target_os = "macos"))]
mod unsupported;
#[cfg(not(target_os = "macos"))]
use unsupported as imp;

/// Exclusive locks, owner-only files, durable renames: the POSIX ones on any
/// Unix, whatever its adapter.
#[cfg(unix)]
#[path = "unix.rs"]
pub mod files;
#[cfg(not(unix))]
pub use unsupported::files;

// Without a feature nothing serves or connects, so the listener goes unused.
#[cfg(test)]
#[cfg_attr(not(any(feature = "server", feature = "client")), allow(dead_code))]
pub(crate) mod memory;

// ── 1. Endpoint ──────────────────────────────────────────────────────────────

/// A connected byte stream between keyd and one client.
pub struct Conn(pub(crate) Stream);

pub(crate) enum Stream {
    Os(imp::Stream),
    #[cfg(test)]
    Memory(memory::Half),
}

impl Conn {
    /// Bounds each read and write from now on; `None` waits for ever. Clients
    /// set one per call; keyd sets none, because a forwarded parse or embed
    /// takes minutes.
    pub fn set_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match &self.0 {
            Stream::Os(s) => imp::set_timeout(s, timeout),
            #[cfg(test)]
            Stream::Memory(m) => m.set_timeout(timeout),
        }
    }
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.0 {
            Stream::Os(s) => s.read(buf),
            #[cfg(test)]
            Stream::Memory(m) => m.read(buf),
        }
    }
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut self.0 {
            Stream::Os(s) => s.write(buf),
            #[cfg(test)]
            Stream::Memory(m) => m.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.0 {
            Stream::Os(s) => s.flush(),
            #[cfg(test)]
            Stream::Memory(m) => m.flush(),
        }
    }
}

/// Where keyd accepts connections.
pub struct Listener(pub(crate) Accept);

pub(crate) enum Accept {
    Os(imp::Listener),
    #[cfg(test)]
    #[cfg_attr(not(any(feature = "server", feature = "client")), allow(dead_code))]
    Memory(memory::Acceptor),
}

impl Listener {
    /// Binds the endpoint named `path` (`paths::socket`), replacing a stale
    /// one, reachable by this user only. For keyd's debug `serve-local` and
    /// for tests; a running keyd gets its listeners from `activated`.
    pub fn bind(path: &Path) -> io::Result<Listener> {
        imp::bind(path).map(|l| Listener(Accept::Os(l)))
    }

    /// Blocks until a client connects.
    pub fn accept(&self) -> io::Result<Conn> {
        loop {
            if let Some(conn) = accept_within(std::slice::from_ref(self), None)? {
                return Ok(conn);
            }
        }
    }
}

/// Waits up to `within` for a client on any of `listeners`, and accepts one;
/// `Ok(None)` when none came. Nothing is accepted that is not returned, so a
/// connection that arrives while keyd decides to exit stays queued at the
/// endpoint for the next keyd.
pub fn accept_any(listeners: &[Listener], within: Duration) -> io::Result<Option<Conn>> {
    accept_within(listeners, Some(within))
}

fn accept_within(listeners: &[Listener], within: Option<Duration>) -> io::Result<Option<Conn>> {
    #[cfg(test)]
    if let [Listener(Accept::Memory(m))] = listeners {
        return m.accept_within(within);
    }
    let os: Vec<&imp::Listener> = listeners
        .iter()
        .filter_map(|l| match &l.0 {
            Accept::Os(l) => Some(l),
            #[cfg(test)]
            Accept::Memory(_) => None,
        })
        .collect();
    imp::accept_any(&os, within).map(|s| s.map(|s| Conn(Stream::Os(s))))
}

/// Why `connect` found no keyd. `Absent`: nothing is there (no endpoint, or
/// nothing listening on it), so keyd is not installed. `Broken`: anything
/// else, a sandbox's refusal included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    Absent(String),
    Broken(String),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::Absent(d) | ConnectError::Broken(d) => f.write_str(d),
        }
    }
}

/// Connects to keyd's endpoint at `path`. Where activation is set up, this
/// starts keyd.
pub fn connect(path: &Path) -> Result<Conn, ConnectError> {
    imp::connect(path).map(|s| Conn(Stream::Os(s)))
}

// ── 2. Activation ────────────────────────────────────────────────────────────

/// The listeners the OS handed keyd when a connect started it.
pub fn activated() -> io::Result<Vec<Listener>> {
    imp::activated().map(|ls| ls.into_iter().map(|l| Listener(Accept::Os(l))).collect())
}

// ── 3. PeerCheck ─────────────────────────────────────────────────────────────

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
    imp::peer_check(policy)
}

// ── 4, 5. The OS secret store ────────────────────────────────────────────────

/// The master key's item: read, or created on first use.
#[cfg(feature = "server")]
pub fn master_key() -> Box<dyn crate::vault::KeySource> {
    imp::master_key()
}

/// The per-service items that predate the vault, for import on use.
#[cfg(feature = "server")]
pub fn legacy_items() -> Box<dyn crate::vault::LegacySource> {
    imp::legacy_items()
}

// ── 6. Registrar ─────────────────────────────────────────────────────────────

/// The registration that starts keyd on a connect: the program it runs and
/// whether the OS has it loaded now.
#[cfg(feature = "client")]
#[derive(Debug, Clone)]
pub struct Registration {
    /// Where the registration lives (a file, on every OS so far).
    pub path: PathBuf,
    /// The keyd it runs, when one is registered.
    pub program: Option<String>,
    pub loaded: bool,
}

#[cfg(feature = "client")]
pub trait Registrar: Sync {
    /// Refuses, before anything is written, a `data_dir` keyd cannot serve
    /// from here.
    fn check(&self, data_dir: &Path) -> Result<(), String>;
    /// Whether `program` must be registered where it is (inside an install,
    /// for the caller check) rather than copied to `paths::installed_bin`.
    fn runs_in_place(&self, program: &Path) -> bool;
    /// Registers `program` to serve `data_dir`, replacing any registration
    /// and leaving it loaded. Returns `Registration::path`.
    fn install(&self, program: &Path, data_dir: &Path) -> Result<PathBuf, String>;
    fn status(&self) -> Result<Registration, String>;
    /// Unloads keyd and removes the registration; returns what it removed.
    fn uninstall(&self) -> Result<Vec<PathBuf>, String>;
    /// Unloads and removes the registration named `label`, one an earlier
    /// Oculus made for something else; returns what it removed, nothing when
    /// there is none. Refuses keyd's own label.
    fn retire(&self, label: &str) -> Result<Vec<PathBuf>, String>;
}

#[cfg(feature = "client")]
pub fn registrar() -> &'static dyn Registrar {
    imp::registrar()
}

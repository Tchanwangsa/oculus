use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

use super::imp;
#[cfg(test)]
use super::memory;

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

/// The listeners the OS handed keyd when a connect started it.
pub fn activated() -> io::Result<Vec<Listener>> {
    imp::activated().map(|ls| ls.into_iter().map(|l| Listener(Accept::Os(l))).collect())
}

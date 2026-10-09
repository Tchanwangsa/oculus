//! Every target without an adapter. Each seam refuses with an `Unsupported`
//! error, `connect` finds keyd `Absent` (so a client goes its no-keyd way),
//! and keyd's `main` exits saying so. It exists so core, keyd and the app
//! still build, which is what proves no OS call has leaked out of
//! `platform/`. On a Unix the file helpers are the POSIX ones (`unix.rs`).

use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

use super::ConnectError;

fn unsupported() -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, format!("oculus-keyd has no adapter for {}", std::env::consts::OS))
}

/// No endpoint can exist, so no value of either type does.
pub(crate) enum Stream {}
pub(crate) enum Listener {}

impl Read for Stream {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        match *self {}
    }
}

impl Write for Stream {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        match *self {}
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {}
    }
}

pub(crate) fn set_timeout(stream: &Stream, _timeout: Option<Duration>) -> io::Result<()> {
    match *stream {}
}

pub(crate) fn bind(_path: &Path) -> io::Result<Listener> {
    Err(unsupported())
}

pub(crate) fn accept_any(_listeners: &[&Listener], _within: Option<Duration>) -> io::Result<Option<Stream>> {
    Err(unsupported())
}

pub(crate) fn connect(_path: &Path) -> Result<Stream, ConnectError> {
    Err(ConnectError::Absent(unsupported().to_string()))
}

pub(crate) fn activated() -> io::Result<Vec<Listener>> {
    Err(unsupported())
}

#[cfg(feature = "server")]
mod server {
    use crate::platform::{Caller, Conn, PeerCheck};
    use crate::vault::{KeyError, KeySource, LegacySource, MasterKey};

    pub(crate) struct NoPeers;

    impl PeerCheck for NoPeers {
        fn inspect(&self, _conn: &Conn) -> Caller {
            Caller { problems: vec![super::unsupported().to_string()], ..Caller::default() }
        }

        fn admit(&self, _caller: &Caller) -> Result<(), String> {
            Err(super::unsupported().to_string())
        }
    }

    pub(crate) struct NoStore;

    impl KeySource for NoStore {
        fn get_or_create(&self) -> Result<MasterKey, KeyError> {
            Err(KeyError::Platform(super::unsupported().to_string()))
        }
    }

    impl LegacySource for NoStore {
        fn read(&self, _service: &str, _account: &str) -> Result<Option<String>, KeyError> {
            Err(KeyError::Platform(super::unsupported().to_string()))
        }
    }
}

#[cfg(feature = "server")]
pub(crate) fn peer_check(_policy: super::Policy) -> Box<dyn super::PeerCheck> {
    Box::new(server::NoPeers)
}

#[cfg(feature = "server")]
pub(crate) fn master_key() -> Box<dyn crate::vault::KeySource> {
    Box::new(server::NoStore)
}

#[cfg(feature = "server")]
pub(crate) fn legacy_items() -> Box<dyn crate::vault::LegacySource> {
    Box::new(server::NoStore)
}

#[cfg(feature = "client")]
struct NoRegistrar;

#[cfg(feature = "client")]
impl super::Registrar for NoRegistrar {
    fn check(&self, _data_dir: &Path) -> Result<(), String> {
        Err(unsupported().to_string())
    }

    fn runs_in_place(&self, _program: &Path) -> bool {
        false
    }

    fn install(&self, _program: &Path, _data_dir: &Path) -> Result<std::path::PathBuf, String> {
        Err(unsupported().to_string())
    }

    fn status(&self) -> Result<super::Registration, String> {
        Err(unsupported().to_string())
    }

    /// Nothing can have been installed.
    fn uninstall(&self) -> Result<Vec<std::path::PathBuf>, String> {
        Ok(Vec::new())
    }
}

#[cfg(feature = "client")]
pub(crate) fn registrar() -> &'static dyn super::Registrar {
    &NoRegistrar
}

/// The file helpers where there is no POSIX: each refuses.
#[cfg(not(unix))]
pub mod files {
    use std::fs::File;
    use std::io;
    use std::path::Path;

    pub struct FileLock;

    pub fn lock(_path: &Path) -> io::Result<FileLock> {
        Err(super::unsupported())
    }

    pub fn create_private(_path: &Path) -> io::Result<File> {
        Err(super::unsupported())
    }

    pub fn sync_dir(_dir: &Path) -> io::Result<()> {
        Err(super::unsupported())
    }

    pub fn set_executable(_path: &Path) -> io::Result<()> {
        Err(super::unsupported())
    }

    pub fn replace_file(_dest: &Path, _fill: impl FnOnce(&Path) -> io::Result<()>) -> Result<(), String> {
        Err(super::unsupported().to_string())
    }
}

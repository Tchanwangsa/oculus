//! The macOS adapter. The endpoint is a unix socket that launchd owns (mode
//! 0600) and hands to keyd on the first connect (`activated`); the peer check
//! reads the caller's audit token and code signature (`peer.rs`); the secret
//! store is the login keychain (`keychain.rs`); the registrar writes and
//! loads a LaunchAgent (`registrar.rs`). Its build step is the reproducible,
//! ad-hoc signed build in `app/scripts/build-keyd.mjs`.

use std::ffi::{c_char, c_int, CString};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::Duration;

use super::ConnectError;
#[cfg(feature = "server")]
use super::{PeerCheck, Policy};

#[cfg(feature = "server")]
mod keychain;
#[cfg(feature = "server")]
mod peer;
#[cfg(feature = "client")]
mod registrar;

pub(crate) type Stream = UnixStream;
pub(crate) type Listener = UnixListener;

/// The plist's `Sockets` key that names keyd's listener.
const SOCKETS_KEY: &str = "Listeners";

pub(crate) fn set_timeout(stream: &UnixStream, timeout: Option<Duration>) -> io::Result<()> {
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)
}

/// Non-blocking, like launchd's listener, so `accept_any` is the same for both.
pub(crate) fn bind(path: &Path) -> io::Result<UnixListener> {
    std::fs::remove_file(path).ok();
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// `poll` on the (non-blocking) listeners, then `accept` on a ready one. A
/// client that left between the two is no connection: `Ok(None)`.
pub(crate) fn accept_any(
    listeners: &[&UnixListener],
    within: Option<Duration>,
) -> io::Result<Option<UnixStream>> {
    let mut fds: Vec<libc::pollfd> = listeners
        .iter()
        .map(|l| libc::pollfd {
            fd: l.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();
    let timeout = within.map_or(-1, |d| d.as_millis().min(c_int::MAX as u128) as c_int);
    if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) } < 0 {
        return Err(io::Error::last_os_error());
    }
    for (fd, listener) in fds.iter().zip(listeners) {
        if fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
            return Err(io::Error::other(format!(
                "listener {} failed (poll revents {:#x})",
                fd.fd, fd.revents
            )));
        }
        if fd.revents & libc::POLLIN == 0 {
            continue;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                // accept() inherits O_NONBLOCK from the listener.
                stream.set_nonblocking(false)?;
                return Ok(Some(stream));
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}

/// ENOENT (no socket) and ECONNREFUSED (a socket file with nothing loaded
/// behind it) are `Absent`; anything else, the sandbox's EPERM included, is
/// `Broken`.
pub(crate) fn connect(path: &Path) -> Result<UnixStream, ConnectError> {
    UnixStream::connect(path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => {
            ConnectError::Absent(e.to_string())
        }
        _ => ConnectError::Broken(e.to_string()),
    })
}

// launch.h, in libSystem. Returns 0 or an errno; the caller frees the array.
extern "C" {
    fn launch_activate_socket(
        name: *const c_char,
        fds: *mut *mut c_int,
        cnt: *mut libc::size_t,
    ) -> c_int;
}

/// The listener launchd made from the plist's `Sockets`, non-blocking (as
/// launchd hands it over) for `accept_any`.
pub(crate) fn activated() -> io::Result<Vec<UnixListener>> {
    let name = CString::new(SOCKETS_KEY).unwrap();
    let mut fds: *mut c_int = std::ptr::null_mut();
    let mut cnt: libc::size_t = 0;
    let rc = unsafe { launch_activate_socket(name.as_ptr(), &mut fds, &mut cnt) };
    if rc != 0 {
        return Err(io::Error::other(format!(
            "launch_activate_socket failed: {}",
            io::Error::from_raw_os_error(rc)
        )));
    }
    if cnt == 0 || fds.is_null() {
        return Err(io::Error::other(
            "launch_activate_socket returned no sockets",
        ));
    }
    let raw = unsafe { std::slice::from_raw_parts(fds, cnt) }.to_vec();
    unsafe { libc::free(fds as *mut libc::c_void) };
    raw.into_iter()
        .map(|fd| {
            let listener = unsafe { UnixListener::from_raw_fd(fd) };
            listener.set_nonblocking(true)?;
            Ok(listener)
        })
        .collect()
}

#[cfg(feature = "server")]
pub(crate) fn peer_check(policy: Policy) -> Box<dyn PeerCheck> {
    Box::new(peer::Check { policy })
}

#[cfg(feature = "server")]
pub(crate) fn master_key() -> Box<dyn crate::vault::KeySource> {
    Box::new(keychain::MasterItem)
}

#[cfg(feature = "server")]
pub(crate) fn legacy_items() -> Box<dyn crate::vault::LegacySource> {
    Box::new(keychain::LegacyKeychain)
}

#[cfg(feature = "client")]
pub(crate) fn registrar() -> &'static dyn super::Registrar {
    &registrar::Launchd
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{self, Listener as AnyListener};

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("keyd-mac-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn no_socket_or_no_listener_is_absent() {
        let dir = scratch("absent");
        let sock = dir.join("keyd.sock");
        assert!(matches!(
            platform::connect(&sock),
            Err(ConnectError::Absent(_))
        ));

        // A socket file launchd left behind with nothing loaded refuses the connect.
        drop(UnixListener::bind(&sock).unwrap());
        assert!(matches!(
            platform::connect(&sock),
            Err(ConnectError::Absent(_))
        ));

        // A directory where the socket should be is no keyd and no absence either.
        std::fs::remove_file(&sock).unwrap();
        std::fs::create_dir(&sock).unwrap();
        assert!(matches!(
            platform::connect(&sock),
            Err(ConnectError::Broken(_))
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_bound_endpoint_is_owner_only_and_carries_bytes_both_ways() {
        use std::io::{Read, Write};
        let dir = scratch("bind");
        let sock = dir.join("keyd.sock");
        std::fs::write(&sock, "stale").unwrap();
        let listener = AnyListener::bind(&sock).unwrap();
        assert_eq!(
            std::fs::metadata(&sock).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let mut client = platform::connect(&sock).unwrap();
        let mut server = listener.accept().unwrap();
        client.write_all(b"hi").unwrap();
        let mut buf = [0u8; 2];
        server.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"hi");

        client.set_timeout(Some(Duration::from_millis(50))).unwrap();
        let err = client.read(&mut buf).unwrap_err();
        assert!(
            matches!(
                err.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ),
            "{err:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

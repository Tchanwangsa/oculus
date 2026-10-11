//! A pseudo-terminal for a CLI that signs in only on a TTY (`agy`).

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};

/// Spawn `cmd` with a fresh PTY as its stdin, stdout, stderr and controlling
/// terminal, sized `cols` × `rows`; returns the child and the master side.
/// The child leads its own session, so its pid is also its process group.
/// `cmd` is consumed: its copies of the slave must close in this process,
/// or the master never sees the child hang up.
pub(super) fn spawn(mut cmd: Command, cols: u16, rows: u16) -> io::Result<(Child, File)> {
    let mut master = -1;
    let mut slave = -1;
    let mut size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: both out-pointers are valid; a null name and termios ask for
    // the defaults.
    let rc = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openpty returned two fresh descriptors that nothing else owns.
    let (master, slave) = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    // openpty's descriptors are inheritable; every other process the app
    // spawns would otherwise hold the terminal open.
    for fd in [&master, &slave] {
        set_cloexec(fd)?;
    }

    cmd.stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave));
    // SAFETY: login_tty only calls setsid, ioctl and dup2, which are
    // async-signal-safe. It makes fd 0 (the slave, by now) the controlling
    // terminal of a new session.
    unsafe {
        cmd.pre_exec(|| {
            if libc::login_tty(0) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = cmd.spawn()?;
    drop(cmd);
    Ok((child, File::from(master)))
}

fn set_cloexec(fd: &OwnedFd) -> io::Result<()> {
    // SAFETY: plain fcntl on a descriptor this function borrows.
    let rc = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) };
    if rc == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Wait up to `timeout_ms` for `file` to be readable. `false` on timeout.
pub(super) fn readable(file: &File, timeout_ms: i32) -> io::Result<bool> {
    let mut pfd = libc::pollfd {
        fd: file.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one valid pollfd.
    let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    match rc {
        -1 => {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                Ok(false)
            } else {
                Err(e)
            }
        }
        0 => Ok(false),
        // POLLHUP counts as readable: the read that follows reports the end.
        _ => Ok(true),
    }
}

/// SIGKILL a whole process group — `agy` and anything it started, such as
/// its `open`.
pub(super) fn kill_group(pgid: u32) {
    // SAFETY: plain kill(2); a group that is already gone is ESRCH, ignored.
    unsafe {
        libc::killpg(pgid as libc::pid_t, libc::SIGKILL);
    }
}

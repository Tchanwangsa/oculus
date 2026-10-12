//! `pty` where there are no pseudo-terminals (Windows): Antigravity's in-app
//! sign-in is refused, and its Settings row points at a terminal instead.

use std::fs::File;
use std::io;
use std::process::{Child, Command};

pub(super) fn spawn(_cmd: Command, _cols: u16, _rows: u16) -> io::Result<(Child, File)> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "sign in to Antigravity from a terminal on Windows",
    ))
}

pub(super) fn readable(_file: &File, _timeout_ms: i32) -> io::Result<bool> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn kill_group(_pgid: u32) {}

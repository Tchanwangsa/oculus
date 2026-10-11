//! The two marker files beside the sign-in's attempt record. `authenticated`
//! says a session Canvas accepted is believed held, so the app's startup
//! probe looks at it; `signed-out` stands every automatic sign-in down until a
//! session is established again. keyd writes them (its sign-in, `session_mark`,
//! `sign_out`); the app and CLI only read them.

use std::io;
use std::path::Path;

use crate::paths;

/// Whether a session Canvas accepted is believed held.
pub fn is_authenticated(data_dir: &Path) -> bool {
    paths::authenticated(data_dir).exists()
}

/// Whether the last sign-out has not been followed by a session.
pub fn is_signed_out(data_dir: &Path) -> bool {
    paths::signed_out(data_dir).exists()
}

/// A session Canvas accepted is held: sets the flag and lifts the signed-out
/// marker. Every path that establishes a session ends here, or the startup
/// probe ignores it. Failures are not reported: the session itself is saved.
pub fn mark_authenticated(data_dir: &Path) {
    let flag = paths::authenticated(data_dir);
    if let Some(parent) = flag.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&flag, b"1").ok();
    std::fs::remove_file(paths::signed_out(data_dir)).ok();
}

/// Removes the flag; true when it was there.
pub fn clear_authenticated(data_dir: &Path) -> io::Result<bool> {
    remove(&paths::authenticated(data_dir))
}

/// Stands every automatic sign-in down until `mark_authenticated`.
pub fn mark_signed_out(data_dir: &Path) -> io::Result<()> {
    let marker = paths::signed_out(data_dir);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&marker, b"1")
}

fn remove(path: &Path) -> io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    #[test]
    fn the_flag_and_the_signed_out_marker_follow_a_sign_out_and_a_session() {
        let dir = Scratch::new("markers");
        assert!(!is_authenticated(&dir.0) && !is_signed_out(&dir.0));
        mark_authenticated(&dir.0);
        assert!(is_authenticated(&dir.0));

        assert!(clear_authenticated(&dir.0).unwrap());
        assert!(!clear_authenticated(&dir.0).unwrap());
        mark_signed_out(&dir.0).unwrap();
        assert!(is_signed_out(&dir.0) && !is_authenticated(&dir.0));

        mark_authenticated(&dir.0);
        assert!(is_authenticated(&dir.0) && !is_signed_out(&dir.0));
    }
}

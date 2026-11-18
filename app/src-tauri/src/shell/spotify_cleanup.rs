//! Deletes what an install that played Spotify still holds: its login in
//! keyd (`music.spotify`) and the music settings rows. The `music` row is the
//! sign that there is anything to delete, and it goes with the others, so
//! after one run every later launch finds nothing and does nothing. A keyd
//! that is absent or refuses only logs: the rows still go.
//!
//! Blocking (a database read, then keyd): runs on the startup thread, off the
//! async runtime.

use keyd_core::names::MUSIC_SPOTIFY;

use crate::db::store;
use crate::providers::credentials::{Credentialed, KeydError};

/// The row whose presence says the install played Spotify.
const SIGN: &str = "music";

/// Every settings row music kept: its switch and output, what last played,
/// and the recent tracks.
const ROWS: &[&str] = &[SIGN, "music_last_played", "music_recent_tracks"];

/// What one run did.
#[derive(Debug, PartialEq, Eq)]
enum Cleaned {
    /// No `music` row: nothing to delete.
    Nothing,
    /// The rows are gone; the keyd delete may have failed and been logged.
    Rows,
}

pub fn run() {
    let keyd = Credentialed::at(&crate::library::paths::data_dir());
    let cleaned = clean(
        store::setting_blocking,
        || keyd.delete(MUSIC_SPOTIFY),
        store::delete_settings_blocking,
    );
    if let Err(why) = cleaned {
        elog!("[oculus] could not delete the Spotify settings rows: {why}");
    }
}

/// The decision, over `read` (a settings row), `forget` (the keyd delete)
/// and `delete` (the settings rows), so tests drive it without a database.
fn clean(
    read: impl FnOnce(&str) -> Option<String>,
    forget: impl FnOnce() -> Result<bool, KeydError>,
    delete: impl FnOnce(&[&str]) -> Result<(), String>,
) -> Result<Cleaned, String> {
    if read(SIGN).is_none() {
        return Ok(Cleaned::Nothing);
    }
    if let Err(why) = forget() {
        elog!("[oculus] could not delete the stored Spotify login from keyd: {why}");
    }
    delete(ROWS)?;
    Ok(Cleaned::Rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn never_forget() -> Result<bool, KeydError> {
        panic!("keyd is not asked without the music row")
    }

    fn never_delete(_: &[&str]) -> Result<(), String> {
        panic!("no row is deleted without the music row")
    }

    #[test]
    fn without_the_music_row_nothing_is_touched() {
        let asked = RefCell::new(Vec::new());
        let read = |key: &str| {
            asked.borrow_mut().push(key.to_string());
            None
        };
        assert_eq!(
            clean(read, never_forget, never_delete),
            Ok(Cleaned::Nothing)
        );
        assert_eq!(*asked.borrow(), ["music"]);
    }

    #[test]
    fn with_the_music_row_the_login_and_every_music_row_go() {
        let forgot = RefCell::new(false);
        let deleted = RefCell::new(Vec::new());
        let cleaned = clean(
            |_| Some(r#"{"enabled":true}"#.into()),
            || {
                *forgot.borrow_mut() = true;
                Ok(true)
            },
            |keys| {
                deleted
                    .borrow_mut()
                    .extend(keys.iter().map(|k| k.to_string()));
                Ok(())
            },
        );
        assert_eq!(cleaned, Ok(Cleaned::Rows));
        assert!(*forgot.borrow());
        assert_eq!(
            *deleted.borrow(),
            ["music", "music_last_played", "music_recent_tracks"]
        );
    }

    #[test]
    fn a_keyd_failure_still_deletes_the_rows() {
        for failure in [KeydError::Absent, KeydError::Broken("x".into())] {
            let deleted = RefCell::new(false);
            let cleaned = clean(
                |_| Some(String::new()),
                || Err(failure),
                |_| {
                    *deleted.borrow_mut() = true;
                    Ok(())
                },
            );
            assert_eq!(cleaned, Ok(Cleaned::Rows));
            assert!(*deleted.borrow());
        }
    }

    #[test]
    fn a_failed_row_delete_is_reported() {
        let cleaned = clean(
            |_| Some(String::new()),
            || Ok(false),
            |_| Err("locked".into()),
        );
        assert_eq!(cleaned, Err("locked".into()));
    }
}

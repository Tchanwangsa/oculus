//! How the `oculus` CLI notices that someone signed in through the app. The
//! sign-in is a SAML flow in a window, so it happens in another process, and
//! the only thing that crosses is what oculus-keyd reports.

use keyd_core::client::SessionStatus;

use super::session;
use crate::credentials::Credentialed;

/// Whether `now` shows a sign-in made since `before`: a Canvas session held
/// and the app marked signed in, where one of those was not true before, or
/// keyd's change count differs. keyd's count restarts when keyd does, so a
/// restart between the two readings can hide a sign-in that lands on the same
/// number; the flag flipping on still shows it. A Canvas session rotating
/// also changes the count, so the caller confirms with Canvas.
pub fn signed_in_since(before: &SessionStatus, now: &SessionStatus) -> bool {
    let held = |s: &SessionStatus| s.canvas && s.authenticated;
    held(now) && (!held(before) || before.generation != now.generation)
}

/// Polls keyd for a sign-in made after it was created.
#[derive(Debug)]
pub struct SignInWatch {
    keyd: Credentialed,
    seen: SessionStatus,
    /// The reading `poll` reported as a sign-in.
    latest: Option<SessionStatus>,
}

impl SignInWatch {
    /// Takes the baseline. `Err` when keyd cannot be asked: no sign-in could
    /// be stored without it.
    pub fn begin(data_dir: &std::path::Path) -> Result<Self, String> {
        let keyd = Credentialed::at(data_dir);
        let seen = keyd.session_status().map_err(|e| e.to_string())?;
        Ok(Self {
            keyd,
            seen,
            latest: None,
        })
    }

    /// True when a sign-in has landed since the baseline. A change that is
    /// not one (a half-stored sign-in, a rotation) moves the baseline up to
    /// it, so it is not looked at twice; a sign-in leaves the baseline alone
    /// until `declined` says Canvas did not accept it.
    pub fn poll(&mut self) -> bool {
        let Some(now) = session::status(&self.keyd) else {
            return false;
        };
        if signed_in_since(&self.seen, &now) {
            self.latest = Some(now);
            return true;
        }
        self.seen = now;
        false
    }

    /// Canvas refused what `poll` reported: wait for the next change.
    pub fn declined(&mut self) {
        if let Some(now) = self.latest.take() {
            self.seen = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeKeyd, Scratch};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    fn status(canvas: bool, authenticated: bool, generation: u64) -> SessionStatus {
        SessionStatus {
            canvas,
            sso: false,
            ed: false,
            authenticated,
            signed_out: false,
            generation,
        }
    }

    #[test]
    fn a_sign_in_is_a_held_session_that_is_new_or_marked_since() {
        let signed_out = status(false, false, 3);
        // The window puts the cookie, then marks: neither step alone is enough.
        assert!(!signed_in_since(&signed_out, &status(true, false, 4)));
        assert!(signed_in_since(&signed_out, &status(true, true, 5)));
        // The flag was left on by a session that died: the new put is the sign-in.
        let stale = status(true, true, 3);
        assert!(!signed_in_since(&stale, &status(true, true, 3)));
        assert!(signed_in_since(&stale, &status(true, true, 4)));
        // A restart reset the count to what it was, but the flag flipped on.
        assert!(signed_in_since(
            &status(false, false, 2),
            &status(true, true, 2)
        ));
        // Signing out is not a sign-in.
        assert!(!signed_in_since(&stale, &status(false, false, 4)));
    }

    #[test]
    fn the_watch_reports_a_sign_in_once_and_ignores_changes_that_are_not_one() {
        let dir = Scratch::new("auth-watch");
        let current = Arc::new(Mutex::new((false, false, 0u64)));
        let served = current.clone();
        let _keyd = FakeKeyd::start(&dir, move |_, _| {
            let (canvas, authenticated, generation) = *served.lock().unwrap();
            (
                json!({"canvas": canvas, "sso": false, "ed": false,
                       "authenticated": authenticated, "signed_out": false,
                       "generation": generation}),
                vec![],
            )
        });
        let set = |v: (bool, bool, u64)| *current.lock().unwrap() = v;

        let mut watch = SignInWatch::begin(&dir).unwrap();
        assert!(!watch.poll());
        set((true, false, 1));
        assert!(!watch.poll(), "the cookie landed, the mark has not");
        set((true, true, 2));
        assert!(watch.poll());
        assert!(watch.poll(), "still a sign-in until Canvas answers");
        watch.declined();
        assert!(!watch.poll(), "the same state is not looked at twice");
        set((true, true, 3));
        assert!(watch.poll(), "a later change is");
    }

    #[test]
    fn without_keyd_there_is_nothing_to_watch() {
        let dir = Scratch::new("auth-watch-absent");
        assert!(SignInWatch::begin(&dir)
            .unwrap_err()
            .contains("not installed"));
    }
}

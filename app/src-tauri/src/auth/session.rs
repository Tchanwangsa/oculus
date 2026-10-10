//! What the app asks oculus-keyd about the Canvas sign-in: the authenticated
//! flag, the sessions a login window or browser tab hands over, and sign-out.
//! Every function takes the client, so a fake keyd can stand in; none blocks
//! on anything but that socket, so call them off the main thread.

use std::sync::atomic::{AtomicBool, Ordering};

use keyd_core::client::{KeydError, SessionKind as Kind, SessionStatus};

use crate::providers::credentials::Credentialed;

static ABSENT_LOGGED: AtomicBool = AtomicBool::new(false);

/// Logs why a keyd call did not go through. An absent keyd is logged once per
/// run: it is the same answer to every call after the first.
pub(crate) fn report(what: &str, error: &KeydError) {
    if matches!(error, KeydError::Absent) && ABSENT_LOGGED.swap(true, Ordering::SeqCst) {
        return;
    }
    eprintln!("[oculus] {what}: {error}");
}

/// What keyd says about the sessions, or `None` (logged) when it cannot be
/// asked: an absent keyd never reads as signed in.
pub(crate) fn status(keyd: &Credentialed) -> Option<SessionStatus> {
    keyd.session_status()
        .map_err(|e| report("could not read the session status", &e))
        .ok()
}

/// Whether keyd believes the app holds a session Canvas accepted.
pub(crate) fn signed_in(keyd: &Credentialed) -> bool {
    status(keyd).is_some_and(|s| s.authenticated)
}

/// Hands keyd the sessions a webview signed in with. An empty header is
/// skipped, never put; a failed put is logged. True when the Canvas session
/// was stored.
pub(crate) fn store_snapshot(keyd: &Credentialed, snapshot: &[(Kind, String)]) -> bool {
    let mut canvas = false;
    for (kind, header) in snapshot {
        if header.is_empty() {
            continue;
        }
        match keyd.session_put(*kind, header) {
            Ok(()) => {
                canvas |= *kind == Kind::Canvas;
                eprintln!(
                    "[oculus] saved {} cookies ({} bytes)",
                    kind.wire(),
                    header.len()
                );
            }
            Err(e) => report(&format!("could not save the {} cookies", kind.wire()), &e),
        }
    }
    canvas
}

/// Records that a person signed in, which lifts a sign-out. keyd's own
/// sign-ins record themselves.
pub(crate) fn mark_signed_in(keyd: &Credentialed) -> Result<(), KeydError> {
    keyd.session_mark(true)
}

/// Records that the app found its session dead.
pub(crate) fn mark_session_dead(keyd: &Credentialed) {
    if let Err(e) = keyd.session_mark(false) {
        report("could not clear the authenticated flag", &e);
    }
}

/// Drops every session and the flag, and stands automatic sign-in down. With
/// no keyd there is no session anywhere to drop. True when there was one.
pub fn sign_out(keyd: &Credentialed) -> Result<bool, String> {
    match keyd.sign_out() {
        Err(KeydError::Absent) => Ok(false),
        other => other.map_err(|e| e.to_string()),
    }
}

/// Whether the app counts as signed in: keyd's flag, or the in-memory state
/// a sign-in this run set. The flag fills the memory. A keyd that cannot be
/// asked contributes nothing.
pub(crate) fn authenticated(keyd: &Credentialed, memory: &std::sync::Mutex<bool>) -> bool {
    let flag = signed_in(keyd);
    let mut remembered = memory.lock().unwrap();
    if flag {
        *remembered = true;
    }
    flag || *remembered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeKeyd, Scratch};
    use serde_json::json;
    use std::sync::Mutex;

    fn status_reply(authenticated: bool) -> serde_json::Value {
        json!({"canvas": true, "sso": false, "ed": false, "authenticated": authenticated,
               "signed_out": false, "generation": 4})
    }

    #[test]
    fn only_non_empty_headers_are_put_and_canvas_reports_whether_it_landed() {
        let dir = Scratch::new("auth-store");
        let keyd = FakeKeyd::start(&dir, |_, _| (json!({"stored": true}), vec![]));
        let client = Credentialed::at(&dir);
        let stored = store_snapshot(
            &client,
            &[
                (Kind::Canvas, "canvas_session=a".into()),
                (Kind::Sso, String::new()),
            ],
        );
        assert!(stored);
        assert_eq!(keyd.ops(), ["session_put"]);
        assert_eq!(keyd.requests()[0].0["kind"], "canvas");
        assert_eq!(keyd.requests()[0].0["value"], "canvas_session=a");

        assert!(!store_snapshot(&client, &[(Kind::Canvas, String::new())]));
        assert!(!store_snapshot(&client, &[(Kind::Sso, "idx=b".into())]));
        assert_eq!(
            keyd.ops(),
            ["session_put", "session_put"],
            "the SSO put only"
        );
    }

    #[test]
    fn a_failed_put_is_not_a_stored_session() {
        let dir = Scratch::new("auth-store-refused");
        let _keyd = FakeKeyd::start(&dir, |_, _| {
            (
                json!({"error": "vault", "detail": "vault.bin is damaged"}),
                vec![],
            )
        });
        let client = Credentialed::at(&dir);
        assert!(!store_snapshot(
            &client,
            &[(Kind::Canvas, "canvas_session=a".into())]
        ));
        // A value keyd cannot carry is refused by the client, without a round trip.
        assert!(!store_snapshot(
            &client,
            &[(Kind::Canvas, "a=\u{e9}".into())]
        ));
    }

    #[test]
    fn with_no_keyd_nothing_is_signed_in_and_nothing_panics() {
        let dir = Scratch::new("auth-absent");
        let client = Credentialed::at(&dir);
        assert!(status(&client).is_none());
        assert!(!signed_in(&client));
        assert!(!store_snapshot(
            &client,
            &[(Kind::Canvas, "canvas_session=a".into())]
        ));
        assert!(matches!(mark_signed_in(&client), Err(KeydError::Absent)));
        mark_session_dead(&client);
        assert_eq!(sign_out(&client), Ok(false));
        let memory = Mutex::new(false);
        assert!(!authenticated(&client, &memory));
        assert!(!*memory.lock().unwrap());
    }

    #[test]
    fn the_flag_or_memory_says_signed_in_and_the_flag_fills_memory() {
        let dir = Scratch::new("auth-status");
        let _keyd = FakeKeyd::start(&dir, |_, _| (status_reply(true), vec![]));
        let client = Credentialed::at(&dir);
        let memory = Mutex::new(false);
        assert!(authenticated(&client, &memory));
        assert!(*memory.lock().unwrap());

        let dir = Scratch::new("auth-status-off");
        let _keyd = FakeKeyd::start(&dir, |_, _| (status_reply(false), vec![]));
        let client = Credentialed::at(&dir);
        assert!(!authenticated(&client, &Mutex::new(false)));
        assert!(
            authenticated(&client, &Mutex::new(true)),
            "memory alone counts"
        );
    }

    #[test]
    fn marking_goes_through_session_mark_both_ways() {
        let dir = Scratch::new("auth-mark");
        let keyd = FakeKeyd::start(&dir, |_, _| (json!({"authenticated": true}), vec![]));
        let client = Credentialed::at(&dir);
        mark_signed_in(&client).unwrap();
        mark_session_dead(&client);
        let sent: Vec<_> = keyd.requests().into_iter().map(|(h, _)| h).collect();
        assert_eq!(
            sent[0],
            json!({"op": "session_mark", "authenticated": true})
        );
        assert_eq!(
            sent[1],
            json!({"op": "session_mark", "authenticated": false})
        );
    }

    #[test]
    fn sign_out_asks_keyd_and_a_refusal_surfaces() {
        let dir = Scratch::new("auth-sign-out");
        let keyd = FakeKeyd::start(&dir, |_, _| (json!({"had": true}), vec![]));
        assert_eq!(sign_out(&Credentialed::at(&dir)), Ok(true));
        assert_eq!(keyd.ops(), ["sign_out"]);

        let dir = Scratch::new("auth-sign-out-refused");
        let _keyd = FakeKeyd::start(&dir, |_, _| {
            (
                json!({"error": "vault", "detail": "vault.bin is damaged"}),
                vec![],
            )
        });
        assert!(sign_out(&Credentialed::at(&dir))
            .unwrap_err()
            .contains("damaged"));
    }
}

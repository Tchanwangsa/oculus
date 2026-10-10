//! The login sessions an earlier version kept in files: Canvas's and Okta's
//! cookie headers and Ed's token. The first session op, session-route forward
//! or sign-in after keyd starts moves each into the vault, once, and deletes
//! the file. The vault's `keyd.imported.session.<kind>` marker is the record
//! that a kind has been dealt with, so a cleared session is never brought
//! back from an old file.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use super::{OpError, State};
use crate::names;
use crate::paths;
use crate::session::{check_value, Kind};
use crate::vault::Vault;

fn file_of(data_dir: &std::path::Path, kind: Kind) -> PathBuf {
    match kind {
        Kind::Canvas => paths::cookie(data_dir),
        Kind::Sso => paths::sso_cookie(data_dir),
        Kind::Ed => paths::ed_token(data_dir),
    }
}

impl State {
    /// Imports each kind not yet marked from its old file, unless the vault
    /// already holds that session (then the file is only retired). A file that
    /// is unreadable or not a usable session is dropped with a log line: it
    /// is not retried, so it cannot come back later over a newer session.
    /// Cheap after the first success in this process.
    pub(super) fn import_sessions(&self, vault: &Vault) -> Result<(), OpError> {
        if self.sessions_imported.load(Ordering::SeqCst) {
            return Ok(());
        }
        let _one = self.importing.lock().unwrap_or_else(|p| p.into_inner());
        let entries = vault.load()?;
        let pending: Vec<Kind> = Kind::ALL
            .into_iter()
            .filter(|k| !entries.contains(&names::imported(k.secret())))
            .collect();
        let found: Vec<(Kind, Option<String>)> =
            pending.iter().map(|k| (*k, self.read_legacy(*k))).collect();

        let imported = vault.update(|e| {
            let mut any = false;
            for (kind, value) in &found {
                let marker = names::imported(kind.secret());
                if e.contains(&marker) {
                    continue;
                }
                if let Some(value) = value {
                    if !e.contains(kind.secret()) {
                        e.insert(kind.secret(), value);
                        any = true;
                    }
                }
                e.insert(&marker, "1");
            }
            any
        })?;
        if imported {
            self.generation.bump();
        }
        for kind in pending {
            self.delete_legacy(kind);
        }
        self.sessions_imported.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn read_legacy(&self, kind: Kind) -> Option<String> {
        let path = file_of(&self.data_dir, kind);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => {
                crate::log(&format!("the old {} file was not read: {e}", kind.wire()));
                return None;
            }
        };
        let value = text.trim();
        match check_value(value) {
            Ok(()) => Some(value.to_string()),
            Err(why) => {
                crate::log(&format!(
                    "the old {} file is not usable: {why}",
                    kind.wire()
                ));
                None
            }
        }
    }

    fn delete_legacy(&self, kind: Kind) {
        match std::fs::remove_file(file_of(&self.data_dir, kind)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => crate::log(&format!(
                "the old {} file was not removed: {e}",
                kind.wire()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{call, key, state_in};
    use super::*;
    use crate::test_support::Scratch;
    use serde_json::json;

    fn vault(dir: &Scratch) -> Vault {
        Vault::new(paths::vault(&dir.0), key())
    }

    fn held(dir: &Scratch, kind: Kind) -> Option<String> {
        vault(dir).get(kind.secret()).unwrap()
    }

    fn marked(dir: &Scratch, kind: Kind) -> bool {
        vault(dir).has(&names::imported(kind.secret())).unwrap()
    }

    fn write_old(dir: &Scratch) {
        std::fs::write(paths::cookie(&dir.0), "canvas_session=OLD; _csrf_token=c\n").unwrap();
        std::fs::write(paths::sso_cookie(&dir.0), "sid=OLD-SSO").unwrap();
        std::fs::write(paths::ed_token(&dir.0), "OLD.ED.TOKEN\n").unwrap();
    }

    fn files(dir: &Scratch) -> [bool; 3] {
        [
            paths::cookie(&dir.0).exists(),
            paths::sso_cookie(&dir.0).exists(),
            paths::ed_token(&dir.0).exists(),
        ]
    }

    #[test]
    fn the_first_session_op_imports_each_old_file_once_and_deletes_it() {
        let dir = Scratch::new("legacy-import");
        write_old(&dir);
        let state = state_in(&dir);
        let status = call(&state, "session_status", json!({})).unwrap();
        assert_eq!(status["canvas"], true);
        assert_eq!(status["sso"], true);
        assert_eq!(status["ed"], true);
        assert_eq!(
            held(&dir, Kind::Canvas).as_deref(),
            Some("canvas_session=OLD; _csrf_token=c")
        );
        assert_eq!(held(&dir, Kind::Sso).as_deref(), Some("sid=OLD-SSO"));
        assert_eq!(held(&dir, Kind::Ed).as_deref(), Some("OLD.ED.TOKEN"));
        assert!(Kind::ALL.into_iter().all(|k| marked(&dir, k)));
        assert_eq!(files(&dir), [false; 3]);
        assert_eq!(state.session_generation(), 1, "an import is a change");
    }

    #[test]
    fn an_imported_kind_is_never_imported_again() {
        let dir = Scratch::new("legacy-once");
        write_old(&dir);
        call(&state_in(&dir), "session_status", json!({})).unwrap();
        call(
            &state_in(&dir),
            "session_clear",
            json!({"kinds": ["canvas"]}),
        )
        .unwrap();
        // An old version writes the file again, and keyd restarts.
        std::fs::write(paths::cookie(&dir.0), "canvas_session=BACK").unwrap();
        let state = state_in(&dir);
        let status = call(&state, "session_status", json!({})).unwrap();
        assert_eq!(status["canvas"], false);
        assert_eq!(held(&dir, Kind::Canvas), None);
        assert_eq!(state.session_generation(), 0);
    }

    #[test]
    fn a_session_already_in_the_vault_wins_and_the_file_is_only_retired() {
        let dir = Scratch::new("legacy-vault-wins");
        write_old(&dir);
        // The vault holds Canvas, with no marker yet (an op from before this).
        vault(&dir)
            .update(|e| e.insert(Kind::Canvas.secret(), "canvas_session=NEW"))
            .unwrap();
        let state = state_in(&dir);
        call(&state, "session_status", json!({})).unwrap();
        assert_eq!(
            held(&dir, Kind::Canvas).as_deref(),
            Some("canvas_session=NEW")
        );
        assert!(marked(&dir, Kind::Canvas));
        assert_eq!(files(&dir), [false; 3]);
        assert_eq!(held(&dir, Kind::Sso).as_deref(), Some("sid=OLD-SSO"));
    }

    #[test]
    fn a_session_forward_imports_before_it_reads_the_vault() {
        use crate::forward::Routes;
        use crate::test_support::{Answer, FakeOrigin};

        let dir = Scratch::new("legacy-forward");
        std::fs::write(paths::cookie(&dir.0), "canvas_session=OLD").unwrap();
        let canvas = FakeOrigin::start(|_| Answer {
            status: 200,
            headers: vec![],
            body: b"ok".to_vec(),
        });
        let state = state_in(&dir).with_routes(
            Routes::compiled()
                .with_origin("canvas", &canvas.origin)
                .unwrap(),
        );
        let req = json!({"secret": "canvas", "method": "GET", "path": "/api/x"});
        let reply = state
            .dispatch(&super::super::testing::cli(), "forward", &req, b"")
            .unwrap();
        assert_eq!(reply.body, b"ok");
        assert_eq!(
            canvas.hits()[0].header("cookie"),
            Some("canvas_session=OLD")
        );
        assert!(!paths::cookie(&dir.0).exists());
    }

    #[test]
    fn an_unusable_or_undeletable_file_is_dropped_without_stopping_the_rest() {
        let dir = Scratch::new("legacy-bad");
        // Control character: not a session. A directory: neither readable nor
        // removable as a file.
        std::fs::write(paths::cookie(&dir.0), "a=1\u{1}").unwrap();
        std::fs::create_dir(paths::sso_cookie(&dir.0)).unwrap();
        std::fs::write(paths::ed_token(&dir.0), "GOOD.TOKEN").unwrap();
        let state = state_in(&dir);
        let status = call(&state, "session_status", json!({})).unwrap();
        assert_eq!(status["canvas"], false);
        assert_eq!(status["sso"], false);
        assert_eq!(status["ed"], true);
        assert!(Kind::ALL.into_iter().all(|k| marked(&dir, k)));
        assert!(paths::sso_cookie(&dir.0).is_dir(), "the stuck file stays");
        assert!(!paths::cookie(&dir.0).exists());
        // Marked, so the next keyd does not look again.
        std::fs::write(paths::cookie(&dir.0), "canvas_session=LATE").unwrap();
        call(&state_in(&dir), "session_status", json!({})).unwrap();
        assert_eq!(held(&dir, Kind::Canvas), None);
    }

    #[test]
    fn a_clear_before_any_import_reports_the_old_session_and_leaves_no_file_to_return() {
        let dir = Scratch::new("legacy-clear");
        write_old(&dir);
        let state = state_in(&dir);
        let cleared = call(&state, "session_clear", json!({"kinds": ["canvas"]})).unwrap();
        assert_eq!(cleared, json!({"cleared": ["canvas"]}));
        assert_eq!(held(&dir, Kind::Canvas), None);
        assert_eq!(files(&dir), [false; 3]);
        // The kinds that were not cleared were imported on the way.
        assert!(held(&dir, Kind::Sso).is_some() && held(&dir, Kind::Ed).is_some());
        let out = call(&state_in(&dir), "sign_out", json!({})).unwrap();
        assert_eq!(out, json!({"had": true}));
        std::fs::write(paths::cookie(&dir.0), "canvas_session=BACK").unwrap();
        let status = call(&state_in(&dir), "session_status", json!({})).unwrap();
        assert_eq!(status["canvas"], false);
    }
}

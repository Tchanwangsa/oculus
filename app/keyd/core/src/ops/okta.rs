//! The Okta ops: keyd holds the username, password and TOTP seed and runs
//! the headless sign-in (`crate::okta`) with them.
//!
//! The values live in the vault under `names::OKTA`, written only by
//! `okta_save` and `okta_forget`, always together and always with their
//! import markers, so an old keychain item is never copied back over a save
//! or a forget. Every op is for the app and the CLI: the caller check admits
//! any executable in the install (ffmpeg ships in it), so the role, checked
//! once in `State::dispatch`, is the real gate. A sign-in's outcome is a reply, not an error: the client
//! rebuilds the exact `LoginError`.

use std::sync::{Condvar, Mutex, MutexGuard};

use serde_json::{json, Value};

use super::{OpError, Reply, State};
use crate::names;
use crate::okta::{
    self, outcome_to_wire, CredentialStore, Credentials, Env, LoginError, OktaStatus, Trigger,
};
use crate::platform::Caller;
use crate::vault::Vault;

type Outcome = Result<String, LoginError>;

/// Lets one sign-in run at a time. A caller that arrives while one runs waits
/// for it and takes its outcome, instead of starting a second attempt or
/// being turned away by the attempt guard's wait; only a caller that arrives
/// after the attempt has finished is judged by the guard.
#[derive(Default)]
pub(super) struct Flight {
    state: Mutex<FlightState>,
    done: Condvar,
}

#[derive(Default)]
struct FlightState {
    running: bool,
    /// Counts finished attempts, so a waiter knows its attempt is over even
    /// if another has started since.
    generation: u64,
    last: Option<Outcome>,
}

/// Publishes the attempt's outcome and wakes the waiters however the attempt
/// ends; a panic publishes a failure, so no waiter hangs.
struct Finish<'a> {
    flight: &'a Flight,
    outcome: Option<Outcome>,
}

impl Drop for Finish<'_> {
    fn drop(&mut self) {
        let mut s = self.flight.lock();
        s.running = false;
        s.generation += 1;
        s.last = Some(self.outcome.take().unwrap_or_else(|| {
            Err(LoginError::Unexpected(
                "the sign-in stopped unexpectedly".to_string(),
            ))
        }));
        self.flight.done.notify_all();
    }
}

impl Flight {
    fn lock(&self) -> MutexGuard<'_, FlightState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Runs `attempt` unless one is running, in which case this waits for it
    /// and returns its outcome.
    fn run(&self, attempt: impl FnOnce() -> Outcome) -> Outcome {
        let mut s = self.lock();
        if s.running {
            let seen = s.generation;
            while s.generation == seen {
                s = self.done.wait(s).unwrap_or_else(|p| p.into_inner());
            }
            return s.last.clone().unwrap_or(Err(LoginError::Unexpected(
                "the sign-in left no outcome".to_string(),
            )));
        }
        s.running = true;
        drop(s);

        let mut finish = Finish {
            flight: self,
            outcome: None,
        };
        let outcome = attempt();
        finish.outcome = Some(outcome.clone());
        outcome
    }
}

/// The vault as the sign-in's credential store.
struct VaultStore<'a>(&'a State);

/// A store failure as the text `LoginError::UnreadableCredentials` carries.
fn describe(e: OpError) -> String {
    match e.kind {
        "keychain" => e.detail,
        kind => format!("{kind}: {}", e.detail),
    }
}

impl CredentialStore for VaultStore<'_> {
    fn load(&self) -> Result<Option<Credentials>, String> {
        let vault = self.0.vault().map_err(describe)?;
        self.0.import_okta(&vault).map_err(describe)?;
        let entries = vault.load().map_err(|e| describe(e.into()))?;
        let get = |name: &str| {
            entries
                .get(name)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        };
        Ok(
            match (
                get(names::OKTA_USERNAME),
                get(names::OKTA_PASSWORD),
                get(names::OKTA_TOTP_SECRET),
            ) {
                (Some(username), Some(password), Some(totp_secret)) => Some(Credentials {
                    username,
                    password,
                    totp_secret,
                }),
                _ => None,
            },
        )
    }

    /// Keeps the password's import marker, so the old keychain item (the same
    /// rejected password) is not copied back.
    fn clear_password(&self) -> Result<(), String> {
        let marker = names::imported(names::OKTA_PASSWORD);
        self.0
            .vault()
            .and_then(|v| {
                v.update(|e| {
                    e.remove(names::OKTA_PASSWORD);
                    e.insert(&marker, "1");
                })
                .map_err(OpError::from)
            })
            .map_err(describe)
    }
}

impl State {
    /// Copies each Okta item's old keychain item in, once. Each costs one
    /// keychain prompt, ever; a refusal is a `keychain` error and is retried.
    fn import_okta(&self, vault: &Vault) -> Result<(), OpError> {
        names::OKTA
            .iter()
            .try_for_each(|name| self.import_once(vault, name))
    }

    /// `{username, password, totp_secret}` in, `{"saved": true}` out. Invalid
    /// input is a `request` error carrying the shared message, and writes
    /// nothing. The three values and their markers land in one vault write.
    pub(super) fn okta_save(&self, req: &Value) -> Result<Reply, OpError> {
        let field = |key: &str| {
            req.get(key).and_then(Value::as_str).ok_or_else(|| {
                OpError::new("request", format!("okta_save needs a string \"{key}\""))
            })
        };
        let creds = okta::validate_credentials(
            field("username")?,
            field("password")?,
            field("totp_secret")?,
        )
        .map_err(|why| OpError::new("request", why))?;

        let values = [
            (names::OKTA_USERNAME, creds.username.as_str()),
            (names::OKTA_PASSWORD, creds.password.as_str()),
            (names::OKTA_TOTP_SECRET, creds.totp_secret.as_str()),
        ];
        self.vault()?.update(|e| {
            for (name, value) in values {
                e.insert(name, value);
                e.insert(&names::imported(name), "1");
            }
        })?;
        // New credentials deserve an immediate automatic try.
        if let Err(why) = okta::resume_automatic_sign_in(&self.data_dir) {
            crate::log(&format!(
                "okta_save: the attempt guard was not cleared ({why})"
            ));
        }
        Ok(json!({"saved": true}).into())
    }

    /// Removes all three values; `{"existed": bool}` says whether any was
    /// there. Their markers are set, so nothing is imported afterwards.
    pub(super) fn okta_forget(&self) -> Result<Reply, OpError> {
        let existed = self.vault()?.update(|e| {
            let mut any = false;
            for name in names::OKTA {
                any |= e.remove(name);
                e.insert(&names::imported(name), "1");
            }
            any
        })?;
        Ok(json!({"existed": existed}).into())
    }

    /// What is on file: the username, and whether a password and a seed are.
    pub(super) fn okta_status(&self) -> Result<Reply, OpError> {
        let vault = self.vault()?;
        self.import_okta(&vault)?;
        let entries = vault.load()?;
        let held = |name: &str| entries.get(name).filter(|v| !v.is_empty());
        let status = OktaStatus {
            username: held(names::OKTA_USERNAME).map(str::to_string),
            has_password: held(names::OKTA_PASSWORD).is_some(),
            has_totp: held(names::OKTA_TOTP_SECRET).is_some(),
        };
        Ok(status.to_wire().into())
    }

    /// Signs in (or waits for the sign-in already running) and replies with
    /// its outcome. The attempt guard, the log and both cookie files are
    /// `okta::sign_in`'s.
    pub(super) fn ensure_signed_in(&self, caller: &Caller, req: &Value) -> Result<Reply, OpError> {
        let trigger = req
            .get("trigger")
            .and_then(Value::as_str)
            .and_then(Trigger::from_wire_name)
            .ok_or_else(|| {
                OpError::new(
                    "request",
                    "ensure_signed_in needs a \"trigger\": manual, startup, keep-alive or browser",
                )
            })?;
        let outcome = self.flight.run(|| {
            let store = VaultStore(self);
            let mut env = Env::new(&self.data_dir, &self.canvas_base, &store);
            if let Some(sso) = &self.sso_base {
                env.sso_base = sso.clone();
            }
            env.now = self.clock.clone();
            okta::sign_in(&env, trigger, caller.role)
        });
        let note = match &outcome {
            Ok(_) => "result=signed_in".to_string(),
            Err(e) => format!("result=error code={}", e.code()),
        };
        Ok(Reply {
            header: outcome_to_wire(&outcome),
            body: Vec::new(),
            note: Some(note),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::okta::outcome_from_wire;
    use crate::platform::Role;
    use crate::test_support::okta_fake::{
        answer, code_from_t0, script, COOKIE, PASSWORD, SEED, T0, USERNAME,
    };
    use crate::test_support::{FakeOrigin, OldItems, Reads, Scratch, TestClock, BUILD};
    use crate::vault::{KeyError, MasterKey, NoLegacy, StaticKey};

    fn key() -> MasterKey {
        MasterKey::from_bytes([7; 32])
    }

    fn as_role(role: Role) -> Caller {
        Caller {
            role,
            ..Caller::default()
        }
    }

    fn cli() -> Caller {
        as_role(Role::Cli)
    }

    fn fake() -> FakeOrigin {
        FakeOrigin::start(script(code_from_t0))
    }

    /// A keyd whose sign-in clock reads `clock`.
    fn state_on(
        dir: &Scratch,
        fake: Option<&FakeOrigin>,
        legacy: Box<dyn crate::vault::LegacySource>,
        clock: &TestClock,
    ) -> State {
        let state = State::new(BUILD, dir.0.clone(), Box::new(StaticKey(key())), legacy)
            .with_clock(clock.clock());
        let Some(fake) = fake else { return state };
        let port = fake.origin.rsplit(':').next().unwrap();
        state
            .with_origins(
                Some(&format!("http://127.0.0.1:{port}")),
                Some(&format!("http://localhost:{port}")),
            )
            .unwrap()
    }

    fn state_with(
        dir: &Scratch,
        fake: Option<&FakeOrigin>,
        legacy: Box<dyn crate::vault::LegacySource>,
    ) -> State {
        state_on(dir, fake, legacy, &TestClock::at(T0))
    }

    fn state(dir: &Scratch, fake: &FakeOrigin) -> State {
        state_with(dir, Some(fake), Box::new(NoLegacy))
    }

    /// A keyd on a clock the test moves.
    fn clocked(dir: &Scratch, fake: &FakeOrigin) -> (State, TestClock) {
        let clock = TestClock::at(T0);
        (state_on(dir, Some(fake), Box::new(NoLegacy), &clock), clock)
    }

    fn op(state: &State, caller: &Caller, name: &str, req: Value) -> Result<Value, OpError> {
        state.dispatch(caller, name, &req, b"").map(|r| r.header)
    }

    fn save(state: &State, username: &str, password: &str, seed: &str) -> Result<Value, OpError> {
        op(
            state,
            &cli(),
            "okta_save",
            json!({"username": username, "password": password, "totp_secret": seed}),
        )
    }

    fn save_good(state: &State) {
        save(state, USERNAME, PASSWORD, SEED).unwrap();
    }

    fn ensure_as(
        state: &State,
        role: Role,
        trigger: &str,
    ) -> Result<Result<(), LoginError>, OpError> {
        let header = op(
            state,
            &as_role(role),
            "ensure_signed_in",
            json!({"trigger": trigger}),
        )?;
        Ok(outcome_from_wire(&header).unwrap_or_else(|| panic!("{header}")))
    }

    fn ensure(state: &State, trigger: &str) -> Result<Result<(), LoginError>, OpError> {
        ensure_as(state, Role::Cli, trigger)
    }

    fn status(state: &State) -> OktaStatus {
        OktaStatus::from_wire(&op(state, &cli(), "okta_status", json!({})).unwrap()).unwrap()
    }

    fn vault_of(dir: &Scratch) -> Vault {
        Vault::new(crate::paths::vault(&dir.0), key())
    }

    fn stored(dir: &Scratch, name: &str) -> Option<String> {
        vault_of(dir).get(name).unwrap()
    }

    fn marked(dir: &Scratch, name: &str) -> bool {
        vault_of(dir).has(&names::imported(name)).unwrap()
    }

    fn requests(fake: &FakeOrigin) -> usize {
        fake.hits().len()
    }

    // ── Who may ask ──────────────────────────────────────────────────────────

    fn good_requests() -> Vec<(&'static str, Value)> {
        vec![
            (
                "okta_save",
                json!({"username": USERNAME, "password": PASSWORD, "totp_secret": SEED}),
            ),
            ("okta_forget", json!({})),
            ("okta_status", json!({})),
            ("ensure_signed_in", json!({"trigger": "manual"})),
        ]
    }

    #[test]
    fn an_unknown_role_is_refused_by_every_okta_op_and_nothing_runs() {
        let dir = Scratch::new("okta-role");
        let fake = fake();
        let state = state(&dir, &fake);
        for (name, req) in good_requests() {
            let err = op(&state, &as_role(Role::Unknown), name, req).unwrap_err();
            assert_eq!(err.kind, "caller", "{name}");
            assert!(!err.detail.contains(PASSWORD), "{name}");
        }
        assert!(
            !crate::paths::vault(&dir.0).exists(),
            "no op touched the vault"
        );
        assert!(!crate::paths::sign_in_record(&dir.0).exists());
        assert_eq!(requests(&fake), 0);
    }

    #[test]
    fn the_app_and_the_cli_are_both_admitted() {
        for role in [Role::App, Role::Cli] {
            let dir = Scratch::new("okta-admit");
            let fake = fake();
            let state = state(&dir, &fake);
            for (name, req) in good_requests() {
                // A manual sign-in reaches the scripted servers and succeeds.
                op(&state, &as_role(role), name, req)
                    .unwrap_or_else(|e| panic!("{role:?} {name}: {e:?}"));
            }
        }
    }

    // ── okta_save ────────────────────────────────────────────────────────────

    #[test]
    fn save_refuses_each_invalid_input_with_the_shared_message_and_writes_nothing() {
        let dir = Scratch::new("okta-invalid");
        let fake = fake();
        let state = state(&dir, &fake);
        let cases = [
            (("  ", PASSWORD, SEED), "Username is required."),
            ((USERNAME, "", SEED), "Password is required."),
            (
                (USERNAME, PASSWORD, ""),
                "That does not look like a TOTP setup key: secret is empty",
            ),
            (
                (USERNAME, PASSWORD, "   "),
                "That does not look like a TOTP setup key: secret is empty",
            ),
            (
                (USERNAME, PASSWORD, "GEZD!NBV"),
                "That does not look like a TOTP setup key: '!' is not a base32 character",
            ),
        ];
        for ((u, p, t), message) in cases {
            let err = save(&state, u, p, t).unwrap_err();
            assert_eq!(
                (err.kind, err.detail.as_str()),
                ("request", message),
                "{u:?} {t:?}"
            );
            assert!(!err.detail.contains(PASSWORD));
        }
        for req in [
            json!({}),
            json!({"username": USERNAME, "password": PASSWORD}),
            json!({"username": USERNAME, "password": 5, "totp_secret": SEED}),
        ] {
            let err = op(&state, &cli(), "okta_save", req).unwrap_err();
            assert_eq!(err.kind, "request");
            assert!(
                err.detail.starts_with("okta_save needs a string"),
                "{}",
                err.detail
            );
        }
        assert!(!crate::paths::vault(&dir.0).exists(), "nothing was written");
    }

    #[test]
    fn a_rejected_save_leaves_what_was_saved_before_untouched() {
        let dir = Scratch::new("okta-atomic");
        let fake = fake();
        let state = state(&dir, &fake);
        save_good(&state);
        assert!(save(&state, "someone-else", PASSWORD, "not base32 !").is_err());
        assert_eq!(
            stored(&dir, names::OKTA_USERNAME).as_deref(),
            Some(USERNAME)
        );
        assert_eq!(stored(&dir, names::OKTA_TOTP_SECRET).as_deref(), Some(SEED));
    }

    #[test]
    fn a_save_writes_all_three_and_their_markers_in_one_vault_update() {
        let dir = Scratch::new("okta-save");
        let fake = fake();
        let state = state(&dir, &fake);
        let reply = save(
            &state,
            "  s1234567 ",
            PASSWORD,
            "gezd gnbv gy3t qojq gezd gnbv gy3t qojq",
        )
        .unwrap();
        assert_eq!(reply, json!({"saved": true}));
        // Trimmed, and the seed with its spaces removed.
        assert_eq!(
            stored(&dir, names::OKTA_USERNAME).as_deref(),
            Some("s1234567")
        );
        assert_eq!(
            stored(&dir, names::OKTA_PASSWORD).as_deref(),
            Some(PASSWORD)
        );
        assert_eq!(
            stored(&dir, names::OKTA_TOTP_SECRET).as_deref(),
            Some("gezdgnbvgy3tqojqgezdgnbvgy3tqojq")
        );
        for name in names::OKTA {
            assert!(marked(&dir, name), "{name}");
        }
        let all = vault_of(&dir).load().unwrap();
        assert_eq!(all.names().count(), 6);
    }

    #[test]
    fn saving_clears_the_attempt_guards_wait_and_pause() {
        let dir = Scratch::new("okta-resume");
        let fake = fake();
        let state = state(&dir, &fake);
        save_good(&state);
        let record = crate::paths::sign_in_record(&dir.0);
        // A lockout, and the last attempt just over a minute ago.
        std::fs::write(
            &record,
            format!(
                r#"{{"last":{},"failures":3,"paused":"locked","manual_failures":3}}"#,
                T0 - 61
            ),
        )
        .unwrap();
        assert!(matches!(
            ensure(&state, "startup").unwrap(),
            Err(LoginError::Paused(_))
        ));
        save_good(&state);
        let text = std::fs::read_to_string(&record).unwrap();
        assert!(
            text.contains(r#""failures":0"#)
                && text.contains(r#""paused":null"#)
                && text.contains(r#""manual_failures":0"#),
            "{text}"
        );
        // So the next automatic attempt runs.
        assert!(matches!(ensure(&state, "startup").unwrap(), Ok(_)));
    }

    #[test]
    fn nothing_a_save_does_shows_a_value() {
        let dir = Scratch::new("okta-leak");
        let fake = fake();
        let state = state(&dir, &fake);
        let reply = state
            .dispatch(
                &cli(),
                "okta_save",
                &json!({"username": USERNAME, "password": PASSWORD, "totp_secret": SEED}),
                b"",
            )
            .unwrap();
        let shown = format!("{} {:?}", reply.header, reply.note);
        for secret in [PASSWORD, SEED] {
            assert!(!shown.contains(secret), "{shown}");
        }
        let sealed = std::fs::read(crate::paths::vault(&dir.0)).unwrap();
        assert!(!sealed
            .windows(PASSWORD.len())
            .any(|w| w == PASSWORD.as_bytes()));
    }

    // ── okta_status and okta_forget ──────────────────────────────────────────

    #[test]
    fn status_names_the_username_and_what_is_on_file_but_no_secret() {
        let dir = Scratch::new("okta-status");
        let fake = fake();
        let state = state(&dir, &fake);
        let empty = OktaStatus {
            username: None,
            has_password: false,
            has_totp: false,
        };
        assert_eq!(status(&state), empty);

        save_good(&state);
        let header = op(&state, &cli(), "okta_status", json!({})).unwrap();
        assert_eq!(
            header,
            json!({"username": USERNAME, "has_password": true, "has_totp": true})
        );
        let shown = header.to_string();
        assert!(!shown.contains(PASSWORD) && !shown.contains(SEED));

        vault_of(&dir).remove(names::OKTA_PASSWORD).unwrap();
        assert_eq!(
            status(&state),
            OktaStatus {
                username: Some(USERNAME.into()),
                has_password: false,
                has_totp: true,
            }
        );
    }

    #[test]
    fn forget_removes_all_three_and_says_whether_any_was_there() {
        let dir = Scratch::new("okta-forget");
        let fake = fake();
        let state = state(&dir, &fake);
        save_good(&state);
        let forget = || op(&state, &cli(), "okta_forget", json!({})).unwrap();
        assert_eq!(forget(), json!({"existed": true}));
        assert_eq!(forget(), json!({"existed": false}));
        for name in names::OKTA {
            assert_eq!(stored(&dir, name), None);
            assert!(marked(&dir, name), "{name} keeps its marker");
        }
        assert!(matches!(
            ensure(&state, "manual").unwrap(),
            Err(LoginError::NotConfigured)
        ));
        // Forgetting one value alone still counts.
        save_good(&state);
        vault_of(&dir).remove(names::OKTA_USERNAME).unwrap();
        vault_of(&dir).remove(names::OKTA_PASSWORD).unwrap();
        assert_eq!(forget(), json!({"existed": true}));
    }

    // ── The generic ops leave Okta alone ─────────────────────────────────────

    #[test]
    fn store_and_delete_refuse_the_okta_names_and_has_still_answers() {
        let dir = Scratch::new("okta-generic");
        let fake = fake();
        let state = state(&dir, &fake);
        for name in names::OKTA {
            let err = op(
                &state,
                &cli(),
                "store",
                json!({"secret": name, "value": "x"}),
            )
            .unwrap_err();
            assert_eq!(err.kind, "request", "{name}");
            assert!(err.detail.contains("okta_save"), "{}", err.detail);
            let err = op(&state, &cli(), "delete", json!({"secret": name})).unwrap_err();
            assert_eq!(err.kind, "request", "{name}");
            assert!(err.detail.contains("okta_forget"), "{}", err.detail);
            assert_eq!(
                op(&state, &cli(), "has", json!({"secret": name})).unwrap(),
                json!({"has": false})
            );
        }
        assert!(
            !crate::paths::vault(&dir.0).exists()
                || vault_of(&dir)
                    .load()
                    .unwrap()
                    .names()
                    .all(|n| n.starts_with("keyd.")),
            "no okta value was written"
        );
        save_good(&state);
        assert_eq!(
            op(&state, &cli(), "has", json!({"secret": "okta.password"})).unwrap(),
            json!({"has": true})
        );
        // A caller of no role may not even ask whether a value exists.
        for name in ["okta.password", "voyage"] {
            let err = op(
                &state,
                &as_role(Role::Unknown),
                "has",
                json!({"secret": name}),
            )
            .unwrap_err();
            assert_eq!(err.kind, "caller", "{name}");
        }
    }

    // ── Import on first use ──────────────────────────────────────────────────

    const SERVICE: &str = "com.oculus.unimelb-sso";

    fn old_items(password: Result<Option<String>, KeyError>) -> (Box<OldItems>, Reads) {
        let reads = Reads::default();
        let items = OldItems(
            vec![
                ((SERVICE, "username"), Ok(Some(USERNAME.to_string()))),
                ((SERVICE, "password"), password),
                ((SERVICE, "totp_secret"), Ok(Some(SEED.to_string()))),
            ],
            reads.clone(),
        );
        (Box::new(items), reads)
    }

    #[test]
    fn status_imports_the_three_old_items_once_and_marks_them() {
        let dir = Scratch::new("okta-import-status");
        let (items, reads) = old_items(Ok(Some(PASSWORD.into())));
        let state = state_with(&dir, None, items);
        assert_eq!(
            status(&state),
            OktaStatus {
                username: Some(USERNAME.into()),
                has_password: true,
                has_totp: true,
            }
        );
        assert_eq!(
            reads.load(Ordering::SeqCst),
            3,
            "one read, so one prompt, per item"
        );
        status(&state);
        assert_eq!(reads.load(Ordering::SeqCst), 3);
        assert_eq!(
            stored(&dir, names::OKTA_PASSWORD).as_deref(),
            Some(PASSWORD)
        );
        for name in names::OKTA {
            assert!(marked(&dir, name), "{name}");
        }
    }

    #[test]
    fn a_forget_is_never_undone_by_a_later_import_even_in_a_new_keyd() {
        let dir = Scratch::new("okta-import-forget");
        let (items, _) = old_items(Ok(Some(PASSWORD.into())));
        let state = state_with(&dir, None, items);
        status(&state);
        op(&state, &cli(), "okta_forget", json!({})).unwrap();

        let (items, reads) = old_items(Ok(Some(PASSWORD.into())));
        let fresh = state_with(&dir, None, items);
        assert_eq!(status(&fresh).username, None);
        assert_eq!(
            reads.load(Ordering::SeqCst),
            0,
            "the old items are not even read"
        );
        assert!(matches!(
            ensure(&fresh, "manual").unwrap(),
            Err(LoginError::NotConfigured)
        ));
    }

    #[test]
    fn forgetting_before_any_import_stops_the_import() {
        let dir = Scratch::new("okta-forget-first");
        let (items, reads) = old_items(Ok(Some(PASSWORD.into())));
        let state = state_with(&dir, None, items);
        op(&state, &cli(), "okta_forget", json!({})).unwrap();
        assert_eq!(status(&state).username, None);
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_save_outranks_the_old_items_and_skips_their_read() {
        let dir = Scratch::new("okta-import-save");
        let (items, reads) = old_items(Ok(Some("the-old-password".into())));
        let state = state_with(&dir, None, items);
        save(&state, "new-user", "new-pw", SEED).unwrap();
        assert_eq!(status(&state).username.as_deref(), Some("new-user"));
        assert_eq!(
            stored(&dir, names::OKTA_PASSWORD).as_deref(),
            Some("new-pw")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ensure_signed_in_imports_the_old_items_then_signs_in_with_them() {
        let dir = Scratch::new("okta-import-ensure");
        let fake = fake();
        let (items, reads) = old_items(Ok(Some(PASSWORD.into())));
        let state = state_with(&dir, Some(&fake), items);
        assert_eq!(ensure(&state, "startup").unwrap(), Ok(()));
        assert_eq!(reads.load(Ordering::SeqCst), 3);
        assert_eq!(
            stored(&dir, names::OKTA_USERNAME).as_deref(),
            Some(USERNAME)
        );
    }

    #[test]
    fn a_refused_old_item_is_a_keychain_error_for_status_and_unreadable_for_a_sign_in() {
        let dir = Scratch::new("okta-import-refused");
        let fake = fake();
        let refused = Err(KeyError::Refused("reading password: OSStatus -128".into()));
        let (items, reads) = old_items(refused);
        let state = state_with(&dir, Some(&fake), items);

        let err = op(&state, &cli(), "okta_status", json!({})).unwrap_err();
        assert_eq!(err.kind, "keychain");
        assert!(err.detail.contains("-128"), "{}", err.detail);

        let Err(LoginError::UnreadableCredentials(why)) = ensure(&state, "manual").unwrap() else {
            panic!("a refused read is unreadable, never not-configured");
        };
        assert!(why.contains("-128"), "{why}");
        assert_eq!(requests(&fake), 0);
        assert!(
            reads.load(Ordering::SeqCst) >= 2,
            "a refusal is retried, not recorded"
        );
        // No attempt was made, so none was recorded.
        assert!(!crate::paths::sign_in_record(&dir.0).exists());
    }

    // ── ensure_signed_in ─────────────────────────────────────────────────────

    #[test]
    fn a_sign_in_through_dispatch_writes_both_cookie_files_and_replies_signed_in() {
        let dir = Scratch::new("okta-ensure");
        let fake = fake();
        let state = state(&dir, &fake);
        save_good(&state);

        let reply = state
            .dispatch(
                &cli(),
                "ensure_signed_in",
                &json!({"trigger": "manual"}),
                b"",
            )
            .unwrap();
        assert_eq!(reply.header, json!({"result": "signed_in"}));
        assert_eq!(reply.note.as_deref(), Some("result=signed_in"));
        assert_eq!(
            std::fs::read_to_string(crate::paths::cookie(&dir.0)).unwrap(),
            COOKIE
        );
        assert_eq!(
            std::fs::read_to_string(crate::paths::sso_cookie(&dir.0)).unwrap(),
            "JSESSIONID=js1; sid=sess1"
        );
        #[cfg(unix)]
        for path in [
            crate::paths::cookie(&dir.0),
            crate::paths::sso_cookie(&dir.0),
        ] {
            assert!(crate::platform::files::is_owner_only(&path).unwrap());
        }
        assert_eq!(requests(&fake), 11, "one flow");
        let log = std::fs::read_to_string(crate::paths::sign_in_log(&dir.0)).unwrap();
        assert!(log.trim_end().ends_with("manual: signed in"), "{log}");
        for secret in [PASSWORD, SEED] {
            assert!(!log.contains(secret));
        }
    }

    #[test]
    fn an_unknown_or_missing_trigger_is_a_request_error() {
        let dir = Scratch::new("okta-trigger");
        let fake = fake();
        let state = state(&dir, &fake);
        for req in [
            json!({}),
            json!({"trigger": "forward"}),
            json!({"trigger": 1}),
            json!({"trigger": "app startup"}),
        ] {
            let err = op(&state, &cli(), "ensure_signed_in", req).unwrap_err();
            assert_eq!(err.kind, "request");
        }
        for t in [
            Trigger::Manual,
            Trigger::Startup,
            Trigger::KeepAlive,
            Trigger::Browser,
        ] {
            assert_eq!(Trigger::from_wire_name(t.wire_name()), Some(t));
        }
        assert_eq!(requests(&fake), 0);
    }

    #[test]
    fn a_bad_password_is_an_outcome_that_clears_only_the_vaults_password() {
        let dir = Scratch::new("okta-bad-password");
        let fake = fake();
        let state = state(&dir, &fake);
        save(&state, USERNAME, "not-the-password", SEED).unwrap();

        let reply = state
            .dispatch(
                &cli(),
                "ensure_signed_in",
                &json!({"trigger": "manual"}),
                b"",
            )
            .unwrap();
        assert_eq!(
            reply.header,
            json!({"result": "error", "code": "bad_password", "detail": "Password is incorrect"})
        );
        assert_eq!(
            reply.note.as_deref(),
            Some("result=error code=bad_password")
        );
        assert_eq!(stored(&dir, names::OKTA_PASSWORD), None);
        assert!(
            marked(&dir, names::OKTA_PASSWORD),
            "the marker stays, so no re-import"
        );
        assert_eq!(
            stored(&dir, names::OKTA_USERNAME).as_deref(),
            Some(USERNAME)
        );
        assert_eq!(stored(&dir, names::OKTA_TOTP_SECRET).as_deref(), Some(SEED));
        assert!(!crate::paths::cookie(&dir.0).exists());
        assert_eq!(
            status(&state),
            OktaStatus {
                username: Some(USERNAME.into()),
                has_password: false,
                has_totp: true,
            }
        );
        // With no password there is nothing to replay.
        let before = requests(&fake);
        assert!(matches!(
            ensure(&state, "manual").unwrap(),
            Err(LoginError::NotConfigured)
        ));
        assert_eq!(requests(&fake), before);
    }

    #[test]
    fn the_second_automatic_attempt_waits_and_a_manual_one_waits_a_minute() {
        let dir = Scratch::new("okta-guard");
        let fake = fake();
        let (state, clock) = clocked(&dir, &fake);
        save_good(&state);

        assert_eq!(ensure(&state, "startup").unwrap(), Ok(()));
        let before = requests(&fake);
        assert_eq!(
            ensure(&state, "browser").unwrap(),
            Err(LoginError::Waiting(600))
        );
        assert_eq!(
            ensure(&state, "manual").unwrap(),
            Err(LoginError::Waiting(60))
        );
        assert_eq!(requests(&fake), before, "the wait costs no request");

        clock.advance(60);
        assert_eq!(ensure(&state, "manual").unwrap(), Ok(()));
        assert!(requests(&fake) > before);
        assert_eq!(
            ensure(&state, "manual").unwrap(),
            Err(LoginError::Waiting(60))
        );
    }

    #[test]
    fn a_manual_request_from_the_cli_cannot_lift_a_lockout_but_the_apps_can() {
        let dir = Scratch::new("okta-lockout-roles");
        let fake = fake();
        let (state, clock) = clocked(&dir, &fake);
        save_good(&state);
        let record = crate::paths::sign_in_record(&dir.0);
        std::fs::write(
            &record,
            format!(
                r#"{{"last":{},"failures":1,"paused":"The account is locked or blocked: x","credentials_paused":true,"manual_failures":0}}"#,
                T0 - 7200
            ),
        )
        .unwrap();

        for trigger in ["manual", "startup", "keep-alive", "browser"] {
            let Err(LoginError::Paused(_)) = ensure_as(&state, Role::Cli, trigger).unwrap() else {
                panic!("the CLI's {trigger} request was not paused");
            };
        }
        assert_eq!(requests(&fake), 0, "the CLI cost Okta nothing");
        assert!(ensure_as(&state, Role::App, "startup").unwrap().is_err());

        assert_eq!(ensure_as(&state, Role::App, "manual").unwrap(), Ok(()));
        // The app's success lifted the pause for everyone.
        clock.advance(60);
        assert_eq!(ensure_as(&state, Role::Cli, "manual").unwrap(), Ok(()));
    }

    #[test]
    fn three_failed_manual_requests_hold_the_next_one_until_credentials_are_saved() {
        let dir = Scratch::new("okta-three");
        let fake = fake();
        let (state, clock) = clocked(&dir, &fake);
        // A well-formed seed that is not the account's: every code is wrong.
        save(&state, USERNAME, PASSWORD, "JBSWY3DPEHPK3PXP").unwrap();
        for _ in 0..3 {
            let Err(LoginError::BadTotp(_)) = ensure(&state, "manual").unwrap() else {
                panic!("expected a rejected code");
            };
            clock.advance(60);
        }
        let before = requests(&fake);
        let Err(LoginError::Waiting(secs)) = ensure(&state, "manual").unwrap() else {
            panic!("a fourth manual attempt ran");
        };
        assert_eq!(secs, 6 * 3600 - 60);
        assert_eq!(requests(&fake), before);

        save_good(&state);
        assert_eq!(ensure(&state, "manual").unwrap(), Ok(()));
    }

    #[test]
    fn a_signed_out_marker_stops_an_automatic_attempt_but_not_a_manual_one() {
        let dir = Scratch::new("okta-signed-out");
        let fake = fake();
        let state = state(&dir, &fake);
        save_good(&state);
        let marker = crate::paths::signed_out(&dir.0);
        std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
        std::fs::write(&marker, b"1").unwrap();
        assert!(matches!(
            ensure(&state, "keep-alive").unwrap(),
            Err(LoginError::SignedOut)
        ));
        assert_eq!(requests(&fake), 0);
        assert!(ensure(&state, "manual").unwrap().is_ok());
    }

    // ── Single flight ────────────────────────────────────────────────────────

    /// Slows the first request of a sign-in, so a second caller arrives while
    /// the first is running.
    fn slow(
        inner: impl Fn(&crate::test_support::Hit) -> crate::test_support::Answer + Send + 'static,
    ) -> FakeOrigin {
        FakeOrigin::start(move |hit| {
            if hit.method == "GET" && hit.path == "/login/saml" {
                std::thread::sleep(Duration::from_millis(600));
            }
            inner(hit)
        })
    }

    fn wait_for_a_request(fake: &FakeOrigin) {
        let started = std::time::Instant::now();
        while fake.hits().is_empty() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "no request came"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_caller_arriving_mid_sign_in_waits_and_gets_its_outcome_and_one_flow_runs() {
        let dir = Scratch::new("okta-flight");
        let fake = slow(script(code_from_t0));
        let state = Arc::new(state(&dir, &fake));
        save_good(&state);

        let first = {
            let s = state.clone();
            std::thread::spawn(move || ensure(&s, "startup").unwrap())
        };
        wait_for_a_request(&fake);
        // Without single flight this would be Waiting(600).
        let second = ensure(&state, "browser").unwrap();
        let first = first.join().unwrap();

        assert_eq!(first, Ok(()));
        assert_eq!(second, first);
        assert_eq!(requests(&fake), 11, "exactly one flow's worth of requests");
        let log = std::fs::read_to_string(crate::paths::sign_in_log(&dir.0)).unwrap();
        assert_eq!(log.lines().count(), 1, "{log}");

        // Arriving after it finished, a caller meets the guard.
        assert!(matches!(
            ensure(&state, "browser").unwrap(),
            Err(LoginError::Waiting(_))
        ));
        assert_eq!(requests(&fake), 11);
    }

    #[test]
    fn waiters_share_a_failure_and_only_a_later_caller_sees_the_guards_wait() {
        let dir = Scratch::new("okta-flight-fail");
        let fake = slow(|_| answer(404, &[], "unexpected"));
        let state = Arc::new(state(&dir, &fake));
        save_good(&state);

        let spawn = |trigger: &'static str| {
            let s = state.clone();
            std::thread::spawn(move || ensure(&s, trigger).unwrap())
        };
        let first = spawn("startup");
        wait_for_a_request(&fake);
        let waiters = [spawn("browser"), spawn("keep-alive")];
        let first = first.join().unwrap();
        let first_error = first.clone().unwrap_err();
        assert!(
            !matches!(first_error, LoginError::Waiting(_)),
            "{first_error}"
        );
        for waiter in waiters {
            assert_eq!(waiter.join().unwrap(), first);
        }
        let attempts = requests(&fake);

        assert!(matches!(
            ensure(&state, "browser").unwrap(),
            Err(LoginError::Waiting(_))
        ));
        assert_eq!(requests(&fake), attempts);
    }

    #[test]
    fn a_sign_in_that_panics_fails_its_waiters_instead_of_hanging_them() {
        let flight = Arc::new(Flight::default());
        let first = {
            let f = flight.clone();
            std::thread::spawn(move || {
                f.run(|| {
                    std::thread::sleep(Duration::from_millis(300));
                    panic!("boom")
                })
            })
        };
        std::thread::sleep(Duration::from_millis(100));
        let waiter = flight.run(|| Ok("never".to_string()));
        assert!(
            matches!(waiter, Err(LoginError::Unexpected(_))),
            "{waiter:?}"
        );
        assert!(first.join().is_err());
        // The flight is usable again.
        assert_eq!(
            flight.run(|| Ok("again".to_string())),
            Ok("again".to_string())
        );
    }

    // ── Origins ──────────────────────────────────────────────────────────────

    #[test]
    fn a_test_origin_must_be_loopback_http() {
        let dir = Scratch::new("okta-origins");
        for bad in [
            "https://127.0.0.1:1",
            "http://example.com:1",
            "http://127.0.0.1",
            "http://127.0.0.1:x",
            "http://localhost.evil.test:1",
        ] {
            let built = state_with(&dir, None, Box::new(NoLegacy));
            assert!(built.with_origins(Some(bad), None).is_err(), "{bad}");
            let built = state_with(&dir, None, Box::new(NoLegacy));
            assert!(built.with_origins(None, Some(bad)).is_err(), "{bad}");
        }
        let built = state_with(&dir, None, Box::new(NoLegacy));
        assert_eq!(built.canvas_base, crate::paths::CANVAS_BASE);
        assert!(built.sso_base.is_none());
    }
}

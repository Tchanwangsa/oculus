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
    self, outcome_to_wire, CredentialStore, Credentials, Env, LoginError, OktaStatus, SessionStore,
    Trigger,
};
use crate::platform::{Caller, Role};
use crate::session::{check_value, store, Kind};
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

    /// Runs `work` with no sign-in running and none able to start: it waits
    /// for the one running to finish, so a sign-in that began before cannot
    /// save a session or lift a marker after `work` has cleared them. A
    /// caller that arrives meanwhile waits too, and is told `SignedOut`.
    pub(super) fn exclusively<T>(&self, work: impl FnOnce() -> T) -> T {
        let mut s = self.lock();
        while s.running {
            s = self.done.wait(s).unwrap_or_else(|p| p.into_inner());
        }
        s.running = true;
        drop(s);

        let mut finish = Finish {
            flight: self,
            outcome: None,
        };
        let result = work();
        finish.outcome = Some(Err(LoginError::SignedOut));
        result
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

/// The vault as the sign-in's session store: a session it mints is a `put`,
/// so the generation counts it and a request that was rejected meanwhile can
/// tell. A legacy file is imported first, so it can never replace this one.
struct VaultSessions<'a>(&'a State);

impl SessionStore for VaultSessions<'_> {
    fn put(&self, kind: Kind, value: &str) -> Result<(), String> {
        check_value(value)?;
        let vault = self.0.vault().map_err(describe)?;
        self.0.import_sessions(&vault).map_err(describe)?;
        store::put(&vault, &self.0.generation, kind, value).map_err(|e| e.to_string())
    }
}

impl State {
    /// Signs in (or waits for the sign-in already running) and returns its
    /// outcome. The one way keyd signs in: `ensure_signed_in` and a rejected
    /// `forward` both come here, so there is one attempt at a time and every
    /// one is the guard's.
    pub(super) fn sign_in(&self, trigger: Trigger, role: Role) -> Outcome {
        self.flight.run(|| {
            let (creds, sessions) = (VaultStore(self), VaultSessions(self));
            let mut env = Env::new(&self.data_dir, &self.canvas_base, &creds, &sessions);
            if let Some(sso) = &self.sso_base {
                env.sso_base = sso.clone();
            }
            env.now = self.clock.clone();
            okta::sign_in(&env, trigger, role)
        })
    }

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

    /// Removes all three values and their old keychain items;
    /// `{"existed": bool, "legacy": …}` says whether any value was there and
    /// what became of the old items (`legacy::Removal`). Their markers are
    /// set, so nothing is imported afterwards.
    pub(super) fn okta_forget(&self) -> Result<Reply, OpError> {
        let existed = self.vault()?.update(|e| {
            let mut any = false;
            for name in names::OKTA {
                any |= e.remove(name);
                e.insert(&names::imported(name), "1");
            }
            any
        })?;
        let legacy = self.remove_legacy(names::OKTA);
        Ok(Reply {
            note: Some(format!("legacy={}", legacy.as_str())),
            ..json!({"existed": existed, "legacy": legacy.as_str()}).into()
        })
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

    /// Clears the attempt guard's failures, pause and wait, and repairs a
    /// damaged record. For the app alone: it answers a person's sign-in in the
    /// login window or a browser tab, and lifting a lockout pause is theirs.
    pub(super) fn okta_resume(&self) -> Result<Reply, OpError> {
        okta::resume_automatic_sign_in(&self.data_dir)
            .map_err(|why| OpError::new("record", why))?;
        Ok(json!({"resumed": true}).into())
    }

    /// Signs in (or waits for the sign-in already running) and replies with
    /// its outcome. The attempt guard, the log and the session entries are
    /// `okta::sign_in`'s.
    pub(super) fn ensure_signed_in(&self, caller: &Caller, req: &Value) -> Result<Reply, OpError> {
        let trigger = req
            .get("trigger")
            .and_then(Value::as_str)
            .and_then(Trigger::from_wire_name)
            .ok_or_else(|| {
                OpError::new(
                    "request",
                    "ensure_signed_in needs a \"trigger\": manual, startup, browser or forward",
                )
            })?;
        let outcome = self.sign_in(trigger, caller.role);
        let note = match &outcome {
            Ok(_) => "result=signed_in".to_string(),
            Err(e) => format!("result=error code={}", e.code()),
        };
        Ok(Reply {
            header: outcome_to_wire(&outcome),
            body: Vec::new(),
            note: Some(note),
            stream: None,
        })
    }
}

#[cfg(test)]
mod tests;

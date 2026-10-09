//! The ops keyd answers, and the vault state behind them.
//!
//! The master key is read on the first op that needs the vault, never at
//! start and never for `ping`, so installing keyd prompts for nothing. One
//! mutex makes that read single-flight: a second connection waits for the
//! first one's keychain prompt instead of raising its own, or creating a
//! second key.

use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::{json, Value};
use vault::{names, KeyError, KeySource, MasterKey, Vault, VaultError};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SOURCE_HASH: &str = env!("KEYD_SOURCE_HASH");

pub struct State {
    data_dir: PathBuf,
    keys: Box<dyn KeySource>,
    master: Mutex<Option<MasterKey>>,
}

/// An op's failure as it goes on the wire: `{"error": kind, "detail": …}`.
/// Kinds: `request` (malformed, unknown op or name), `caller` (refused),
/// `keychain` (the master key was refused or failed), `vault`.
#[derive(Debug)]
pub struct OpError {
    pub kind: &'static str,
    pub detail: String,
}

impl OpError {
    pub fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        OpError { kind, detail: detail.into() }
    }

    pub fn to_json(&self) -> Value {
        json!({"error": self.kind, "detail": self.detail})
    }
}

impl From<KeyError> for OpError {
    fn from(e: KeyError) -> Self {
        OpError::new("keychain", e.to_string())
    }
}

impl From<VaultError> for OpError {
    fn from(e: VaultError) -> Self {
        OpError::new("vault", e.to_string())
    }
}

impl State {
    pub fn new(data_dir: PathBuf, keys: Box<dyn KeySource>) -> Self {
        State { data_dir, keys, master: Mutex::new(None) }
    }

    fn master(&self) -> Result<MasterKey, KeyError> {
        let mut cached = self.master.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(key) = cached.as_ref() {
            return Ok(key.clone());
        }
        let key = self.keys.get_or_create()?;
        *cached = Some(key.clone());
        Ok(key)
    }

    fn vault(&self) -> Result<Vault, OpError> {
        Ok(Vault::new(vault::path_in(&self.data_dir), self.master()?))
    }

    /// One request. `body` is the raw bytes after the header line; no stage-1
    /// op takes any. A reply never carries a secret value.
    pub fn dispatch(&self, op: &str, req: &Value, body: &[u8]) -> Result<Value, OpError> {
        if !body.is_empty() {
            return Err(OpError::new("request", format!("{op} takes no body")));
        }
        match op {
            "ping" => Ok(json!({"version": VERSION, "source_hash": SOURCE_HASH, "pid": std::process::id()})),
            "has" => Ok(json!({"has": self.vault()?.has(secret_name(req)?)?})),
            "store" => {
                let name = secret_name(req)?;
                let value = req.get("value").and_then(Value::as_str).ok_or_else(|| OpError::new("request", "store needs a string \"value\""))?;
                if value.is_empty() {
                    return Err(OpError::new("request", "store needs a non-empty value"));
                }
                self.vault()?.store(name, value)?;
                Ok(json!({"stored": true}))
            }
            "delete" => Ok(json!({"existed": self.vault()?.remove(secret_name(req)?)?})),
            _ => Err(OpError::new("request", format!("unknown op {op:?}"))),
        }
    }
}

fn secret_name(req: &Value) -> Result<&str, OpError> {
    let name = req.get("secret").and_then(Value::as_str).ok_or_else(|| OpError::new("request", "missing \"secret\""))?;
    if !names::is_known(name) {
        return Err(OpError::new("request", format!("unknown secret {name:?}")));
    }
    Ok(name)
}

// ── Migration ────────────────────────────────────────────────────────────────

/// Reads one pre-vault keychain item. The keychain in production; a map in tests.
#[cfg_attr(not(test), allow(dead_code))]
pub trait LegacySource {
    fn read(&self, service: &str, account: &str) -> Result<Option<String>, KeyError>;
}

#[allow(dead_code)]
pub struct LegacyKeychain;

impl LegacySource for LegacyKeychain {
    fn read(&self, service: &str, account: &str) -> Result<Option<String>, KeyError> {
        vault::keychain::read_password(service, account)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Migrated {
    pub copied: Vec<&'static str>,
    /// Already in the vault, so the old item was not read.
    pub kept: Vec<&'static str>,
    /// No old item, or an empty one.
    pub absent: Vec<&'static str>,
    pub failed: Vec<(&'static str, String)>,
}

/// Copies each old per-service keychain item into the vault, skipping names
/// the vault already holds. Copy-only: the old items stay, because the app
/// still reads them. Runs when the readers switch over to keyd.
#[cfg_attr(not(test), allow(dead_code))]
pub fn migrate(vault: &Vault, legacy: &dyn LegacySource) -> Result<Migrated, VaultError> {
    let present = vault.load()?;
    let mut out = Migrated::default();
    let mut found = Vec::new();
    for item in names::LEGACY {
        if present.contains(item.secret) {
            out.kept.push(item.secret);
            continue;
        }
        // Each read may prompt, so they run outside the vault's lock.
        match legacy.read(item.service, item.account) {
            Ok(Some(value)) if !value.is_empty() => found.push((item.secret, value)),
            Ok(_) => out.absent.push(item.secret),
            Err(e) => out.failed.push((item.secret, e.to_string())),
        }
    }
    vault.update(|entries| {
        for (name, value) in &found {
            // A store that landed while the old items were read wins.
            if entries.contains(name) {
                out.kept.push(*name);
            } else {
                entries.insert(name, value);
                out.copied.push(*name);
            }
        }
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;
    use std::collections::HashMap;
    use vault::StaticKey;

    fn key() -> MasterKey {
        MasterKey::from_bytes([9; 32])
    }

    struct Counting(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl KeySource for Counting {
        fn get_or_create(&self) -> Result<MasterKey, KeyError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(20));
            Ok(key())
        }
    }

    struct Refusing;

    impl KeySource for Refusing {
        fn get_or_create(&self) -> Result<MasterKey, KeyError> {
            Err(KeyError::Refused("user cancelled".into()))
        }
    }

    #[test]
    fn ping_never_reads_the_master_key() {
        let dir = Scratch::new("ping");
        let state = State::new(dir.0.clone(), Box::new(Refusing));
        let reply = state.dispatch("ping", &json!({"op": "ping"}), b"").unwrap();
        assert_eq!(reply["source_hash"], SOURCE_HASH);
        assert_eq!(reply["source_hash"].as_str().unwrap().len(), 64);
        assert_eq!(reply["pid"], std::process::id());
        assert_eq!(reply["version"], VERSION);
    }

    #[test]
    fn a_refused_master_key_is_a_keychain_error_and_is_retried() {
        let dir = Scratch::new("refused");
        let state = State::new(dir.0.clone(), Box::new(Refusing));
        let err = state.dispatch("has", &json!({"secret": "voyage"}), b"").unwrap_err();
        assert_eq!(err.kind, "keychain");
        assert_eq!(err.to_json()["error"], "keychain");
        assert!(state.master.lock().unwrap().is_none(), "a refusal is not cached");
    }

    #[test]
    fn the_master_key_is_read_once_across_racing_connections() {
        let dir = Scratch::new("single-flight");
        let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let state = std::sync::Arc::new(State::new(dir.0.clone(), Box::new(Counting(reads.clone()))));
        let threads: Vec<_> = (0..6)
            .map(|_| {
                let s = state.clone();
                std::thread::spawn(move || s.dispatch("has", &json!({"secret": "groq"}), b"").unwrap())
            })
            .collect();
        for t in threads {
            assert_eq!(t.join().unwrap()["has"], false);
        }
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn store_has_delete_and_never_echo_a_value() {
        let dir = Scratch::new("ops");
        let state = State::new(dir.0.clone(), Box::new(StaticKey(key())));
        let stored = state.dispatch("store", &json!({"secret": "voyage", "value": "pa-SECRET"}), b"").unwrap();
        assert!(!stored.to_string().contains("pa-SECRET"));
        assert_eq!(state.dispatch("has", &json!({"secret": "voyage"}), b"").unwrap()["has"], true);
        assert_eq!(state.dispatch("delete", &json!({"secret": "voyage"}), b"").unwrap()["existed"], true);
        assert_eq!(state.dispatch("delete", &json!({"secret": "voyage"}), b"").unwrap()["existed"], false);
        assert_eq!(state.dispatch("has", &json!({"secret": "voyage"}), b"").unwrap()["has"], false);
    }

    #[test]
    fn malformed_requests_are_request_errors() {
        let dir = Scratch::new("bad");
        let state = State::new(dir.0.clone(), Box::new(StaticKey(key())));
        for (op, req, body) in [
            ("get", json!({"secret": "voyage"}), &b""[..]),
            ("has", json!({}), b""),
            ("has", json!({"secret": "session.canvas"}), b""),
            ("store", json!({"secret": "voyage"}), b""),
            ("store", json!({"secret": "voyage", "value": ""}), b""),
            ("store", json!({"secret": "voyage", "value": 5}), b""),
            ("ping", json!({}), b"x"),
        ] {
            let err = state.dispatch(op, &req, body).unwrap_err();
            assert_eq!(err.kind, "request", "{op} {req}");
        }
        assert!(!vault::path_in(&dir.0).exists(), "nothing was written");
    }

    struct Items(HashMap<(&'static str, &'static str), Result<Option<String>, KeyError>>);

    impl LegacySource for Items {
        fn read(&self, service: &str, account: &str) -> Result<Option<String>, KeyError> {
            self.0
                .iter()
                .find(|((s, a), _)| *s == service && *a == account)
                .map(|(_, r)| r.clone())
                .unwrap_or(Ok(None))
        }
    }

    #[test]
    fn migration_copies_missing_names_and_keeps_existing_ones() {
        let dir = Scratch::new("migrate");
        let v = Vault::new(vault::path_in(&dir.0), key());
        v.store(names::GROQ, "gsk-new").unwrap();
        let legacy = Items(HashMap::from([
            (("com.tchan.oculus.voyage", "voyage"), Ok(Some("pa-old".to_string()))),
            (("com.tchan.oculus.groq", "groq"), Ok(Some("gsk-old".to_string()))),
            (("com.tchan.oculus.mineru", "mineru"), Ok(Some(String::new()))),
            (("com.oculus.unimelb-sso", "username"), Ok(Some("student".to_string()))),
            (("com.oculus.unimelb-sso", "password"), Err(KeyError::Refused("denied".into()))),
        ]));

        let m = migrate(&v, &legacy).unwrap();
        assert_eq!(m.copied, [names::VOYAGE, names::OKTA_USERNAME]);
        assert_eq!(m.kept, [names::GROQ]);
        assert_eq!(m.absent, [names::MINERU, names::OKTA_TOTP_SECRET]);
        assert_eq!(m.failed.len(), 1);
        assert_eq!(m.failed[0].0, names::OKTA_PASSWORD);

        assert_eq!(v.get(names::VOYAGE).unwrap().as_deref(), Some("pa-old"));
        assert_eq!(v.get(names::GROQ).unwrap().as_deref(), Some("gsk-new"), "the vault's value wins");
        assert!(!v.has(names::MINERU).unwrap());

        // Idempotent: a second run copies nothing.
        let again = migrate(&v, &legacy).unwrap();
        assert!(again.copied.is_empty());
        assert_eq!(again.kept, [names::VOYAGE, names::GROQ, names::OKTA_USERNAME]);
    }
}

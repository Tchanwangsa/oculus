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

use crate::forward::{Call, Routes, Upstream};
use crate::names;
use crate::platform::Caller;
use crate::vault::{KeyError, KeySource, LegacySource, MasterKey, Vault, VaultError};

/// What `ping` reports: keyd's version and the source hash its build script
/// computed, which only the binary knows.
#[derive(Debug, Clone, Copy)]
pub struct Build {
    pub version: &'static str,
    pub source_hash: &'static str,
}

/// Secrets whose old keychain item keyd copies into the vault the first time
/// an op touches them (`State::import_once`).
const IMPORTED_ON_USE: &[&str] = &[names::VOYAGE];

pub struct State {
    build: Build,
    data_dir: PathBuf,
    keys: Box<dyn KeySource>,
    master: Mutex<Option<MasterKey>>,
    legacy: Box<dyn LegacySource>,
    /// Held across one import, so racing first requests read the old item once.
    importing: Mutex<()>,
    routes: Routes,
    upstream: Upstream,
}

/// An op's failure as it goes on the wire: `{"error": kind, "detail": …}`.
/// Kinds: `request` (malformed, unknown op or name, a refused path or header),
/// `caller` (refused), `keychain` (the master key, or an old item being
/// imported, was refused or failed), `vault`, `missing` (`forward` for a
/// secret the vault does not hold), `upstream` (`forward` got no answer: DNS,
/// connect, TLS or a reset).
#[derive(Debug)]
pub struct OpError {
    pub kind: &'static str,
    pub detail: String,
}

impl OpError {
    pub fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        OpError {
            kind,
            detail: detail.into(),
        }
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

/// A reply: the header line, the raw body after it, and what the log line
/// may add (never a value, a header or a body).
#[derive(Debug)]
pub struct Reply {
    pub header: Value,
    pub body: Vec<u8>,
    pub note: Option<String>,
}

impl From<Value> for Reply {
    fn from(header: Value) -> Self {
        Reply {
            header,
            body: Vec::new(),
            note: None,
        }
    }
}

impl State {
    /// `legacy` is where `import_once` reads old items: the OS secret store
    /// in production, never in tests.
    pub fn new(
        build: Build,
        data_dir: PathBuf,
        keys: Box<dyn KeySource>,
        legacy: Box<dyn LegacySource>,
    ) -> Self {
        State {
            build,
            data_dir,
            keys,
            master: Mutex::new(None),
            legacy,
            importing: Mutex::new(()),
            routes: Routes::compiled(),
            upstream: Upstream::new(),
        }
    }

    /// Debug builds only, like `Routes::with_origin`.
    #[cfg(debug_assertions)]
    pub fn with_routes(mut self, routes: Routes) -> Self {
        self.routes = routes;
        self
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
        Ok(Vault::new(
            crate::paths::vault(&self.data_dir),
            self.master()?,
        ))
    }

    /// One request. `body` is the raw bytes after the header line; only
    /// `forward` takes any. No reply carries a secret value. `_caller` is who
    /// asked: no op checks its role yet, and `forward` is open to any
    /// admitted caller.
    pub fn dispatch(
        &self,
        _caller: &Caller,
        op: &str,
        req: &Value,
        body: &[u8],
    ) -> Result<Reply, OpError> {
        if !body.is_empty() && op != "forward" {
            return Err(OpError::new("request", format!("{op} takes no body")));
        }
        match op {
            "ping" => Ok(json!({"version": self.build.version, "source_hash": self.build.source_hash, "pid": std::process::id()}).into()),
            "has" => {
                let name = secret_name(req)?;
                let vault = self.vault()?;
                self.import_once(&vault, name)?;
                Ok(json!({"has": vault.has(name)?}).into())
            }
            "store" => {
                let name = secret_name(req)?;
                let value = req.get("value").and_then(Value::as_str).ok_or_else(|| OpError::new("request", "store needs a string \"value\""))?;
                if value.is_empty() {
                    return Err(OpError::new("request", "store needs a non-empty value"));
                }
                // Marked imported too: the stored key outranks any old item.
                let marker = imported_marker(name);
                self.vault()?.update(|e| {
                    e.insert(name, value);
                    if let Some(m) = &marker {
                        e.insert(m, "1");
                    }
                })?;
                Ok(json!({"stored": true}).into())
            }
            "delete" => {
                let name = secret_name(req)?;
                // Marked imported, so the old item still in the keychain never comes back.
                let marker = imported_marker(name);
                let existed = self.vault()?.update(|e| {
                    if let Some(m) = &marker {
                        e.insert(m, "1");
                    }
                    e.remove(name)
                })?;
                Ok(json!({"existed": existed}).into())
            }
            "forward" => self.forward(req, body),
            _ => Err(OpError::new("request", format!("unknown op {op:?}"))),
        }
    }

    /// Sends one request to `secret`'s fixed origin with the key added. The
    /// request is checked before the master key is read, so a bad one never
    /// prompts. Any status the origin answers is a reply, not an error.
    fn forward(&self, req: &Value, body: &[u8]) -> Result<Reply, OpError> {
        let name = secret_name(req)?;
        let route = self
            .routes
            .get(name)
            .ok_or_else(|| OpError::new("request", format!("{name} cannot be forwarded")))?;
        let call = Call::parse(req, route, body)?;
        let vault = self.vault()?;
        self.import_once(&vault, name)?;
        let key = vault
            .get(name)?
            .filter(|k| !k.is_empty())
            .ok_or_else(|| OpError::new("missing", format!("no {name} key is stored")))?;
        let answer = self.upstream.send(&route.origin, &call, &key, body)?;
        let headers: Vec<[&str; 2]> = answer
            .headers
            .iter()
            .map(|(k, v)| [k.as_str(), v.as_str()])
            .collect();
        Ok(Reply {
            header: json!({"status": answer.status, "headers": headers, "body_len": answer.body.len()}),
            note: Some(format!(
                "secret={name} status={} bytes_out={} bytes_in={}",
                answer.status,
                body.len(),
                answer.body.len()
            )),
            body: answer.body,
        })
    }

    /// Copies `name`'s old keychain item into the vault once, then marks it
    /// imported. Copy-only: the old item stays, because the app still reads it
    /// when keyd is absent. A value already in the vault wins, and is never
    /// read over. A refused read of the old item is a `keychain` error.
    fn import_once(&self, vault: &Vault, name: &str) -> Result<(), OpError> {
        let Some(marker) = imported_marker(name) else {
            return Ok(());
        };
        let item = names::LEGACY
            .iter()
            .find(|l| l.secret == name)
            .ok_or_else(|| OpError::new("vault", format!("{name} has no old item")))?;
        let _one = self.importing.lock().unwrap_or_else(|p| p.into_inner());
        let entries = vault.load()?;
        if entries.contains(&marker) {
            return Ok(());
        }
        // May prompt, so it runs outside the vault's lock.
        let old = if entries.contains(name) {
            None
        } else {
            self.legacy
                .read(item.service, item.account)
                .map_err(|e| match e {
                    KeyError::Refused(d) | KeyError::Platform(d) => OpError::new("keychain", d),
                })?
        };
        vault.update(|e| {
            // A store or delete that landed meanwhile has already decided.
            if e.contains(&marker) {
                return;
            }
            if !e.contains(name) {
                if let Some(value) = old.as_deref().filter(|v| !v.is_empty()) {
                    e.insert(name, value);
                }
            }
            e.insert(&marker, "1");
        })?;
        Ok(())
    }
}

fn imported_marker(name: &str) -> Option<String> {
    IMPORTED_ON_USE
        .contains(&name)
        .then(|| names::imported(name))
}

fn secret_name(req: &Value) -> Result<&str, OpError> {
    let name = req
        .get("secret")
        .and_then(Value::as_str)
        .ok_or_else(|| OpError::new("request", "missing \"secret\""))?;
    if !names::is_known(name) {
        return Err(OpError::new("request", format!("unknown secret {name:?}")));
    }
    Ok(name)
}

// ── Migration ────────────────────────────────────────────────────────────────

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
    use crate::test_support::{Answer, FakeOrigin, Scratch, BUILD};
    use crate::vault::{NoLegacy, StaticKey};
    use std::collections::HashMap;

    fn key() -> MasterKey {
        MasterKey::from_bytes([9; 32])
    }

    fn state_in(dir: &Scratch) -> State {
        State::new(
            BUILD,
            dir.0.clone(),
            Box::new(StaticKey(key())),
            Box::new(NoLegacy),
        )
    }

    /// The reply's header, for ops that answer without a body.
    fn call(state: &State, op: &str, req: Value) -> Result<Value, OpError> {
        state.dispatch(&Caller::default(), op, &req, b"").map(|r| {
            assert!(r.body.is_empty());
            r.header
        })
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
        let state = State::new(BUILD, dir.0.clone(), Box::new(Refusing), Box::new(NoLegacy));
        let reply = call(&state, "ping", json!({"op": "ping"})).unwrap();
        assert_eq!(reply["source_hash"], BUILD.source_hash);
        assert_eq!(reply["source_hash"].as_str().unwrap().len(), 64);
        assert_eq!(reply["pid"], std::process::id());
        assert_eq!(reply["version"], BUILD.version);
    }

    #[test]
    fn a_refused_master_key_is_a_keychain_error_and_is_retried() {
        let dir = Scratch::new("refused");
        let state = State::new(BUILD, dir.0.clone(), Box::new(Refusing), Box::new(NoLegacy));
        let err = state
            .dispatch(&Caller::default(), "has", &json!({"secret": "voyage"}), b"")
            .unwrap_err();
        assert_eq!(err.kind, "keychain");
        assert_eq!(err.to_json()["error"], "keychain");
        assert!(
            state.master.lock().unwrap().is_none(),
            "a refusal is not cached"
        );
    }

    #[test]
    fn the_master_key_is_read_once_across_racing_connections() {
        let dir = Scratch::new("single-flight");
        let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let state = std::sync::Arc::new(State::new(
            BUILD,
            dir.0.clone(),
            Box::new(Counting(reads.clone())),
            Box::new(NoLegacy),
        ));
        let threads: Vec<_> = (0..6)
            .map(|_| {
                let s = state.clone();
                std::thread::spawn(move || {
                    s.dispatch(&Caller::default(), "has", &json!({"secret": "groq"}), b"")
                        .unwrap()
                        .header
                })
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
        let state = state_in(&dir);
        let stored = call(
            &state,
            "store",
            json!({"secret": "voyage", "value": "pa-SECRET"}),
        )
        .unwrap();
        assert!(!stored.to_string().contains("pa-SECRET"));
        assert_eq!(
            call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
            true
        );
        assert_eq!(
            call(&state, "delete", json!({"secret": "voyage"})).unwrap()["existed"],
            true
        );
        assert_eq!(
            call(&state, "delete", json!({"secret": "voyage"})).unwrap()["existed"],
            false
        );
        assert_eq!(
            call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
            false
        );
    }

    #[test]
    fn malformed_requests_are_request_errors() {
        let dir = Scratch::new("bad");
        let state = state_in(&dir);
        for (op, req, body) in [
            ("get", json!({"secret": "voyage"}), &b""[..]),
            ("has", json!({}), b""),
            ("has", json!({"secret": "session.canvas"}), b""),
            ("store", json!({"secret": "voyage"}), b""),
            ("store", json!({"secret": "voyage", "value": ""}), b""),
            ("store", json!({"secret": "voyage", "value": 5}), b""),
            ("ping", json!({}), b"x"),
        ] {
            let err = state
                .dispatch(&Caller::default(), op, &req, body)
                .unwrap_err();
            assert_eq!(err.kind, "request", "{op} {req}");
        }
        assert!(!crate::paths::vault(&dir.0).exists(), "nothing was written");
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
        let v = Vault::new(crate::paths::vault(&dir.0), key());
        v.store(names::GROQ, "gsk-new").unwrap();
        let legacy = Items(HashMap::from([
            (
                ("com.tchan.oculus.voyage", "voyage"),
                Ok(Some("pa-old".to_string())),
            ),
            (
                ("com.tchan.oculus.groq", "groq"),
                Ok(Some("gsk-old".to_string())),
            ),
            (
                ("com.tchan.oculus.mineru", "mineru"),
                Ok(Some(String::new())),
            ),
            (
                ("com.oculus.unimelb-sso", "username"),
                Ok(Some("student".to_string())),
            ),
            (
                ("com.oculus.unimelb-sso", "password"),
                Err(KeyError::Refused("denied".into())),
            ),
        ]));

        let m = migrate(&v, &legacy).unwrap();
        assert_eq!(m.copied, [names::VOYAGE, names::OKTA_USERNAME]);
        assert_eq!(m.kept, [names::GROQ]);
        assert_eq!(m.absent, [names::MINERU, names::OKTA_TOTP_SECRET]);
        assert_eq!(m.failed.len(), 1);
        assert_eq!(m.failed[0].0, names::OKTA_PASSWORD);

        assert_eq!(v.get(names::VOYAGE).unwrap().as_deref(), Some("pa-old"));
        assert_eq!(
            v.get(names::GROQ).unwrap().as_deref(),
            Some("gsk-new"),
            "the vault's value wins"
        );
        assert!(!v.has(names::MINERU).unwrap());

        // Idempotent: a second run copies nothing.
        let again = migrate(&v, &legacy).unwrap();
        assert!(again.copied.is_empty());
        assert_eq!(
            again.kept,
            [names::VOYAGE, names::GROQ, names::OKTA_USERNAME]
        );
    }

    // ── forward ──────────────────────────────────────────────────────────────

    const KEY: &str = "pa-VAULT-KEY";

    fn forwarding(dir: &Scratch, origin: &FakeOrigin) -> State {
        let state = state_in(dir).with_routes(
            Routes::compiled()
                .with_origin("voyage", &origin.origin)
                .unwrap(),
        );
        call(&state, "store", json!({"secret": "voyage", "value": KEY})).unwrap();
        state
    }

    fn forward(state: &State, body: &[u8]) -> Result<Reply, OpError> {
        let req = json!({
            "op": "forward", "secret": "voyage", "method": "POST", "path": "/v1/multimodalembeddings",
            "headers": [["Content-Type", "application/json"], ["Accept", "application/json"]],
            "body_len": body.len(),
        });
        state.dispatch(&Caller::default(), "forward", &req, body)
    }

    fn header<'a>(reply: &'a Reply, name: &str) -> Option<&'a str> {
        reply.header["headers"]
            .as_array()?
            .iter()
            .find(|h| h[0] == name)
            .and_then(|h| h[1].as_str())
    }

    fn answer(status: u16, headers: Vec<(&'static str, String)>, body: &[u8]) -> Answer {
        Answer {
            status,
            headers,
            body: body.to_vec(),
        }
    }

    #[test]
    fn forward_adds_the_vault_key_and_passes_the_answer_through() {
        let origin = FakeOrigin::start(|_| {
            answer(
                200,
                vec![("Content-Type", "application/json".into())],
                b"{\"usage\":{\"total_tokens\":7}}",
            )
        });
        let dir = Scratch::new("fwd-ok");
        let state = forwarding(&dir, &origin);

        let reply = forward(&state, b"{\"inputs\":[]}").unwrap();
        assert_eq!(reply.header["status"], 200);
        assert_eq!(reply.header["body_len"], reply.body.len());
        assert_eq!(reply.body, b"{\"usage\":{\"total_tokens\":7}}");
        assert_eq!(header(&reply, "content-type"), Some("application/json"));

        let hit = &origin.hits()[0];
        assert_eq!(
            (hit.method.as_str(), hit.path.as_str()),
            ("POST", "/v1/multimodalembeddings")
        );
        assert_eq!(
            hit.header("authorization"),
            Some(format!("Bearer {KEY}").as_str())
        );
        assert_eq!(hit.header("content-type"), Some("application/json"));
        assert_eq!(
            hit.header("accept-encoding"),
            None,
            "no gzip, so the body is the origin's bytes"
        );
        assert_eq!(hit.body, b"{\"inputs\":[]}");

        let shown = format!(
            "{} {} {:?}",
            reply.header,
            String::from_utf8_lossy(&reply.body),
            reply.note
        );
        assert!(!shown.contains(KEY), "{shown}");
        assert!(reply.note.unwrap().contains("status=200"));
    }

    #[test]
    fn error_statuses_come_back_as_replies_byte_for_byte() {
        let bodies: [(u16, &[u8]); 5] = [
            (401, b"{\"detail\":\"Provided API key is invalid.\"}"),
            (402, b"{\"detail\":\"credit\"}"),
            (429, b"{\"detail\":\"3 RPM and 10K TPM\"}"),
            (503, b"<html>upstream \xff</html>"),
            (400, b""),
        ];
        for (status, body) in bodies {
            let origin = FakeOrigin::start(move |_| {
                answer(status, vec![("Retry-After", "17".into())], body)
            });
            let dir = Scratch::new("fwd-status");
            let reply = forward(&forwarding(&dir, &origin), b"{}").unwrap();
            assert_eq!(reply.header["status"], status);
            assert_eq!(reply.body, body);
            assert_eq!(header(&reply, "retry-after"), Some("17"));
        }
    }

    #[test]
    fn a_redirect_is_returned_not_followed() {
        let origin = FakeOrigin::start(|_| {
            answer(
                302,
                vec![("Location", "http://127.0.0.1:1/elsewhere".into())],
                b"",
            )
        });
        let dir = Scratch::new("fwd-redirect");
        let reply = forward(&forwarding(&dir, &origin), b"{}").unwrap();
        assert_eq!(reply.header["status"], 302);
        assert_eq!(
            header(&reply, "location"),
            Some("http://127.0.0.1:1/elsewhere")
        );
        assert_eq!(origin.hits().len(), 1);
    }

    #[test]
    fn no_stored_key_is_missing_and_nothing_is_sent() {
        let origin = FakeOrigin::start(|_| answer(200, vec![], b""));
        let dir = Scratch::new("fwd-missing");
        let state = state_in(&dir).with_routes(
            Routes::compiled()
                .with_origin("voyage", &origin.origin)
                .unwrap(),
        );
        assert_eq!(forward(&state, b"{}").unwrap_err().kind, "missing");
        assert!(origin.hits().is_empty());
    }

    #[test]
    fn an_unreachable_origin_is_upstream_and_names_nothing_sent() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let dir = Scratch::new("fwd-upstream");
        let state = state_in(&dir).with_routes(
            Routes::compiled()
                .with_origin("voyage", &format!("http://127.0.0.1:{port}"))
                .unwrap(),
        );
        call(&state, "store", json!({"secret": "voyage", "value": KEY})).unwrap();
        let err = forward(&state, b"{\"inputs\":\"BODY-TEXT\"}").unwrap_err();
        assert_eq!(err.kind, "upstream");
        for leak in [KEY, "BODY-TEXT", "multimodal", &port.to_string()] {
            assert!(!err.detail.contains(leak), "{leak} in {}", err.detail);
        }
    }

    #[test]
    fn a_bad_forward_is_refused_before_the_master_key_is_read() {
        let dir = Scratch::new("fwd-bad");
        let state = State::new(BUILD, dir.0.clone(), Box::new(Refusing), Box::new(NoLegacy));
        for req in [
            json!({"secret": "voyage", "method": "POST", "path": "/v2/x"}),
            json!({"secret": "voyage", "method": "DELETE", "path": "/v1/x"}),
            json!({"secret": "voyage", "method": "POST", "path": "/v1/x", "headers": [["Authorization", "Bearer x"]]}),
            json!({"secret": "mineru", "method": "POST", "path": "/api/v4/x"}),
            json!({"secret": "okta.password", "method": "POST", "path": "/v1/x"}),
        ] {
            let err = state
                .dispatch(&Caller::default(), "forward", &req, b"")
                .unwrap_err();
            assert_eq!(err.kind, "request", "{req}");
        }
    }

    // ── Import on first use ──────────────────────────────────────────────────

    /// One old Voyage item, counting its reads.
    struct OldVoyage(
        Result<Option<String>, KeyError>,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    );

    impl LegacySource for OldVoyage {
        fn read(&self, service: &str, account: &str) -> Result<Option<String>, KeyError> {
            assert_eq!((service, account), ("com.tchan.oculus.voyage", "voyage"));
            self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.0.clone()
        }
    }

    fn with_old(
        dir: &Scratch,
        old: Result<Option<String>, KeyError>,
    ) -> (State, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let state = State::new(
            BUILD,
            dir.0.clone(),
            Box::new(StaticKey(key())),
            Box::new(OldVoyage(old, reads.clone())),
        );
        (state, reads)
    }

    #[test]
    fn the_old_item_is_imported_once_and_a_delete_is_not_undone() {
        let dir = Scratch::new("import");
        let (state, reads) = with_old(&dir, Ok(Some("pa-old".into())));
        assert_eq!(
            call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
            true
        );
        assert_eq!(
            call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
            true
        );
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
        let v = Vault::new(crate::paths::vault(&dir.0), key());
        assert_eq!(v.get("voyage").unwrap().as_deref(), Some("pa-old"));

        assert_eq!(
            call(&state, "delete", json!({"secret": "voyage"})).unwrap()["existed"],
            true
        );
        assert_eq!(
            call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
            false,
            "the old item stays gone"
        );

        // A fresh keyd over the same vault agrees.
        let (again, reads) = with_old(&dir, Ok(Some("pa-old".into())));
        assert_eq!(
            call(&again, "has", json!({"secret": "voyage"})).unwrap()["has"],
            false
        );
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(
            call(&again, "has", json!({"secret": "keyd.imported.voyage"})).is_err(),
            "the marker is no secret"
        );
    }

    #[test]
    fn a_stored_key_outranks_the_old_item_and_skips_its_read() {
        let dir = Scratch::new("import-store");
        let (state, reads) = with_old(&dir, Ok(Some("pa-old".into())));
        call(
            &state,
            "store",
            json!({"secret": "voyage", "value": "pa-new"}),
        )
        .unwrap();
        assert_eq!(
            call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
            true
        );
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(
            Vault::new(crate::paths::vault(&dir.0), key())
                .get("voyage")
                .unwrap()
                .as_deref(),
            Some("pa-new")
        );
    }

    #[test]
    fn no_old_item_is_imported_as_absent_and_not_read_again() {
        let dir = Scratch::new("import-none");
        let (state, reads) = with_old(&dir, Ok(None));
        assert_eq!(
            call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
            false
        );
        assert_eq!(
            call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
            false
        );
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn a_refused_old_item_is_a_keychain_error_and_is_tried_again() {
        let dir = Scratch::new("import-refused");
        let (state, reads) = with_old(
            &dir,
            Err(KeyError::Refused(
                "reading com.tchan.oculus.voyage/voyage: OSStatus -128".into(),
            )),
        );
        let err = call(&state, "has", json!({"secret": "voyage"})).unwrap_err();
        assert_eq!(err.kind, "keychain");
        assert!(err.detail.contains("-128"), "{}", err.detail);
        assert!(call(&state, "has", json!({"secret": "voyage"})).is_err());
        assert_eq!(
            reads.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "a refusal is not recorded as imported"
        );
    }

    #[test]
    fn forward_imports_the_old_item_first() {
        let origin = FakeOrigin::start(|_| answer(200, vec![], b"{}"));
        let dir = Scratch::new("import-forward");
        let (state, _) = with_old(&dir, Ok(Some("pa-old".into())));
        let state = state.with_routes(
            Routes::compiled()
                .with_origin("voyage", &origin.origin)
                .unwrap(),
        );
        assert_eq!(forward(&state, b"{}").unwrap().header["status"], 200);
        assert_eq!(
            origin.hits()[0].header("authorization"),
            Some("Bearer pa-old")
        );
    }

    #[test]
    fn other_names_are_never_imported() {
        let dir = Scratch::new("import-groq");
        let (state, reads) = with_old(&dir, Ok(Some("x".into())));
        assert_eq!(
            call(&state, "has", json!({"secret": "groq"})).unwrap()["has"],
            false
        );
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}

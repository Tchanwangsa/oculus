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

use crate::clock::Clock;
use crate::forward::{Routes, Upstream};
use crate::names;
use crate::platform::{Caller, Role};
use crate::vault::{KeyError, KeySource, LegacySource, MasterKey, Vault, VaultError};

/// What `ping` reports: keyd's version and the source hash its build script
/// computed, which only the binary knows.
#[derive(Debug, Clone, Copy)]
pub struct Build {
    pub version: &'static str,
    pub source_hash: &'static str,
}

mod forward;
mod legacy;
mod okta;
#[cfg(test)]
mod resign_tests;
mod session;
#[cfg(test)]
mod testing;

/// Secrets whose old keychain item keyd copies into the vault the first time
/// an op touches them (`State::import_once`): the three cloud keys and the
/// three Okta values.
const IMPORTED_ON_USE: &[&str] = &[
    names::VOYAGE,
    names::MINERU,
    names::GROQ,
    names::OKTA_USERNAME,
    names::OKTA_PASSWORD,
    names::OKTA_TOTP_SECRET,
];

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
    /// Canvas's origin for the sign-in; production's unless a debug build
    /// moved it.
    canvas_base: String,
    /// Okta's origin; `None` is production's (`okta::Env::new`).
    sso_base: Option<String>,
    /// One sign-in at a time; see `okta::Flight`.
    flight: okta::Flight,
    /// The time the sign-in and its attempt guard see.
    clock: Clock,
    /// Counts every change to a login session (`session::Generation`).
    generation: crate::session::Generation,
    /// Set once the old session files have been dealt with (`ops/legacy.rs`).
    sessions_imported: std::sync::atomic::AtomicBool,
}

/// An op's failure as it goes on the wire: `{"error": kind, "detail": …}`.
/// Kinds: `request` (malformed, unknown op or name, a refused path or header),
/// `caller` (refused: the peer check, or any op but `ping` asked by a program
/// that is neither the app nor the CLI), `keychain` (the master key, or an old item
/// being imported, was refused or failed), `vault`, `record` (`okta_resume`:
/// the sign-in attempt record could not be replaced; `session_mark` and
/// `sign_out` use it for a marker file they could not write; a client reads
/// it as `Broken`), `missing` (`forward` for a key or session the vault does not hold),
/// `upstream` (`forward` got no answer: DNS, connect, TLS, a reset, or a
/// session route's origin stalled). A `missing` session may also carry the
/// `signin` outcome of the attempt to make one.
#[derive(Debug)]
pub struct OpError {
    pub kind: &'static str,
    pub detail: String,
    /// The refused or failed sign-in beside a `missing` session, in
    /// `outcome_to_wire`'s shape.
    pub signin: Option<Value>,
}

impl OpError {
    pub fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        OpError {
            kind,
            detail: detail.into(),
            signin: None,
        }
    }

    pub fn to_json(&self) -> Value {
        let mut error = json!({"error": self.kind, "detail": self.detail});
        if let Some(signin) = &self.signin {
            error["signin"] = signin.clone();
        }
        error
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
/// may add (never a value, a header or a body). A `stream` replaces the body:
/// the server copies it to the client until it ends, then closes the
/// connection.
#[derive(Debug)]
pub struct Reply {
    pub header: Value,
    pub body: Vec<u8>,
    pub note: Option<String>,
    pub stream: Option<BodyStream>,
}

/// A body that is read as it is sent.
pub struct BodyStream(pub Box<dyn std::io::Read + Send>);

impl std::fmt::Debug for BodyStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BodyStream(..)")
    }
}

impl From<Value> for Reply {
    fn from(header: Value) -> Self {
        Reply {
            header,
            body: Vec::new(),
            note: None,
            stream: None,
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
            canvas_base: crate::paths::CANVAS_BASE.to_string(),
            sso_base: None,
            flight: okta::Flight::default(),
            clock: crate::clock::system(),
            generation: crate::session::Generation::default(),
            sessions_imported: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// How many times a login session has changed since keyd started.
    pub fn session_generation(&self) -> u64 {
        self.generation.get()
    }

    /// Moves the sign-in's clock. Debug builds only, like `with_origins`: a
    /// release keyd always reads the wall clock.
    #[cfg(debug_assertions)]
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// Debug builds only, like `Routes::with_origin`.
    #[cfg(debug_assertions)]
    pub fn with_routes(mut self, routes: Routes) -> Self {
        self.routes = routes;
        self
    }

    /// Points the sign-in, and the `canvas` route, at fake Canvas and Okta
    /// servers: Canvas is one origin to both. Debug builds only, like
    /// `with_routes`: a release keyd cannot move an origin.
    #[cfg(debug_assertions)]
    pub fn with_origins(mut self, canvas: Option<&str>, sso: Option<&str>) -> Result<Self, String> {
        if let Some(origin) = canvas {
            self.canvas_base = test_origin(origin)?;
            self.routes.set_origin(crate::forward::CANVAS, origin);
        }
        if let Some(origin) = sso {
            self.sso_base = Some(test_origin(origin)?);
        }
        Ok(self)
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
    /// `forward` takes any. No reply carries a secret value. `caller` is who
    /// asked: the peer check admits any executable in the install (ffmpeg
    /// ships in it), so every op but `ping` also needs the app or the CLI.
    pub fn dispatch(
        &self,
        caller: &Caller,
        op: &str,
        req: &Value,
        body: &[u8],
    ) -> Result<Reply, OpError> {
        require_role(op, caller.role)?;
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
                refuse_managed(name, true)?;
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
                refuse_managed(name, false)?;
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
            "forward" => self.forward(caller, req, body),
            "okta_save" => self.okta_save(req),
            "okta_forget" => self.okta_forget(),
            "okta_status" => self.okta_status(),
            "ensure_signed_in" => self.ensure_signed_in(caller, req),
            "okta_resume" => self.okta_resume(),
            "session_get" => self.session_get(),
            "session_put" => self.session_put(req),
            "session_clear" => self.session_clear(req),
            "session_status" => self.session_status(),
            "session_mark" => self.session_mark(req),
            "sign_out" => self.sign_out(),
            _ => Err(OpError::new("request", format!("unknown op {op:?}"))),
        }
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

/// The one place an op's caller is judged, so a new op cannot forget it.
/// `ping` answers whoever the peer check admitted, for diagnostics; the rest
/// are for the app and the CLI, except `okta_resume`, which lifts a lockout
/// pause, and `session_get`, the one reply that carries a cookie, so both are
/// for the app alone.
fn require_role(op: &str, role: Role) -> Result<(), OpError> {
    let (allowed, who) = match op {
        "ping" => return Ok(()),
        "okta_resume" | "session_get" => (role == Role::App, "app"),
        _ => (matches!(role, Role::App | Role::Cli), "app and CLI"),
    };
    if allowed {
        return Ok(());
    }
    Err(OpError::new(
        "caller",
        format!("this op is for the Oculus {who} only"),
    ))
}

fn imported_marker(name: &str) -> Option<String> {
    IMPORTED_ON_USE
        .contains(&name)
        .then(|| names::imported(name))
}

/// The generic `store` and `delete` leave the Okta values and the sessions to
/// their own ops, which validate what they write.
fn refuse_managed(name: &str, saving: bool) -> Result<(), OpError> {
    let how = if names::OKTA.contains(&name) {
        if saving {
            "saved with okta_save"
        } else {
            "removed with okta_forget"
        }
    } else if names::SESSIONS.contains(&name) {
        if saving {
            "written with session_put"
        } else {
            "removed with session_clear"
        }
    } else {
        return Ok(());
    };
    Err(OpError::new(
        "request",
        format!("{name} is not written by this op; it is {how}"),
    ))
}

/// A fake service's origin: loopback `http` only, `127.0.0.1` or `localhost`
/// (two hosts, so one server can play both Canvas and Okta).
#[cfg(debug_assertions)]
fn test_origin(origin: &str) -> Result<String, String> {
    let port = origin
        .strip_prefix("http://127.0.0.1:")
        .or_else(|| origin.strip_prefix("http://localhost:"))
        .filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    match port {
        Some(_) => Ok(origin.to_string()),
        None => {
            Err("a test origin must be http://127.0.0.1:<port> or http://localhost:<port>".into())
        }
    }
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

#[cfg(test)]
mod tests {
    use super::testing::{as_role, call, cli, key, state_in};
    use super::*;
    use crate::test_support::{Answer, FakeOrigin, OldItems, Reads, Scratch, BUILD};
    use crate::vault::{NoLegacy, StaticKey};

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
            .dispatch(&cli(), "has", &json!({"secret": "voyage"}), b"")
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
                    s.dispatch(&cli(), "has", &json!({"secret": "groq"}), b"")
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

    /// One well-formed request for every op there is.
    fn every_op() -> Vec<(&'static str, Value)> {
        vec![
            ("has", json!({"secret": "voyage"})),
            ("store", json!({"secret": "voyage", "value": "pa-x"})),
            ("delete", json!({"secret": "voyage"})),
            (
                "forward",
                json!({"secret": "voyage", "method": "POST", "path": "/v1/x"}),
            ),
            (
                "okta_save",
                json!({"username": "u", "password": "p", "totp_secret": "GEZD"}),
            ),
            ("okta_forget", json!({})),
            ("okta_status", json!({})),
            ("ensure_signed_in", json!({"trigger": "manual"})),
            ("okta_resume", json!({})),
            ("session_get", json!({})),
            (
                "session_put",
                json!({"kind": "canvas", "value": "canvas_session=x"}),
            ),
            ("session_clear", json!({})),
            ("session_status", json!({})),
            ("session_mark", json!({"authenticated": true})),
            ("sign_out", json!({})),
            ("a_future_op", json!({})),
        ]
    }

    #[test]
    fn every_op_but_ping_refuses_a_caller_of_no_role_before_anything_runs() {
        let dir = Scratch::new("no-role");
        let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let state = State::new(
            BUILD,
            dir.0.clone(),
            Box::new(Counting(reads.clone())),
            Box::new(NoLegacy),
        );
        let nobody = as_role(Role::Unknown);
        for (op, req) in every_op() {
            let err = state.dispatch(&nobody, op, &req, b"").unwrap_err();
            assert_eq!(err.kind, "caller", "{op}");
            assert!(!err.detail.contains(op), "{op}: {}", err.detail);
        }
        // Not even a body is read for it.
        assert_eq!(
            state
                .dispatch(&nobody, "has", &json!({"secret": "voyage"}), b"x")
                .unwrap_err()
                .kind,
            "caller"
        );
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(!crate::paths::vault(&dir.0).exists());
        assert!(!crate::paths::sign_in_record(&dir.0).exists());

        // ping is the one op that answers everyone, for diagnostics.
        let ping = state.dispatch(&nobody, "ping", &json!({}), b"").unwrap();
        assert_eq!(ping.header["version"], BUILD.version);
    }

    #[test]
    fn the_app_and_the_cli_get_past_the_role_gate_for_every_op() {
        for role in [Role::App, Role::Cli] {
            let dir = Scratch::new("with-role");
            let state = state_in(&dir);
            for (op, req) in every_op() {
                // Whatever else the op says, it is not a refusal of the caller,
                // except that lifting a lockout pause and reading a cookie
                // back are the app's alone.
                let refused = matches!(
                    state.dispatch(&as_role(role), op, &req, b""),
                    Err(OpError { kind: "caller", .. })
                );
                assert_eq!(
                    refused,
                    role == Role::Cli && matches!(op, "okta_resume" | "session_get"),
                    "{role:?} {op}"
                );
            }
        }
    }

    #[test]
    fn malformed_requests_are_request_errors() {
        let dir = Scratch::new("bad");
        let state = state_in(&dir);
        for (op, req, body) in [
            ("get", json!({"secret": "voyage"}), &b""[..]),
            ("has", json!({}), b""),
            ("has", json!({"secret": "session.nope"}), b""),
            ("store", json!({"secret": "voyage"}), b""),
            ("store", json!({"secret": "voyage", "value": ""}), b""),
            ("store", json!({"secret": "voyage", "value": 5}), b""),
            ("ping", json!({}), b"x"),
        ] {
            let err = state.dispatch(&cli(), op, &req, body).unwrap_err();
            assert_eq!(err.kind, "request", "{op} {req}");
        }
        assert!(!crate::paths::vault(&dir.0).exists(), "nothing was written");
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
        state.dispatch(&cli(), "forward", &req, body)
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
            json!({"secret": "mineru", "method": "POST", "path": "/v1/x"}),
            json!({"secret": "groq", "method": "POST", "path": "/api/v4/x"}),
            json!({"secret": "okta.password", "method": "POST", "path": "/v1/x"}),
        ] {
            let err = state.dispatch(&cli(), "forward", &req, b"").unwrap_err();
            assert_eq!(err.kind, "request", "{req}");
        }
    }

    // ── Import on first use ──────────────────────────────────────────────────

    fn with_items(
        dir: &Scratch,
        items: Vec<(
            (&'static str, &'static str),
            Result<Option<String>, KeyError>,
        )>,
    ) -> (State, Reads) {
        let reads = Reads::default();
        let state = State::new(
            BUILD,
            dir.0.clone(),
            Box::new(StaticKey(key())),
            Box::new(OldItems(items, reads.clone())),
        );
        (state, reads)
    }

    /// One old Voyage item.
    fn with_old(dir: &Scratch, old: Result<Option<String>, KeyError>) -> (State, Reads) {
        with_items(dir, vec![(("com.tchan.oculus.voyage", "voyage"), old)])
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
    fn mineru_and_groqs_old_items_are_imported_once() {
        let dir = Scratch::new("import-mineru-groq");
        let (state, reads) = with_items(
            &dir,
            vec![
                (
                    ("com.tchan.oculus.mineru", "mineru"),
                    Ok(Some("mineru-old".into())),
                ),
                (
                    ("com.tchan.oculus.groq", "groq"),
                    Ok(Some("gsk_old".into())),
                ),
            ],
        );
        for name in ["mineru", "groq", "mineru", "groq"] {
            assert_eq!(
                call(&state, "has", json!({"secret": name})).unwrap()["has"],
                true,
                "{name}"
            );
        }
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 2);
        let v = Vault::new(crate::paths::vault(&dir.0), key());
        assert_eq!(v.get("mineru").unwrap().as_deref(), Some("mineru-old"));
        assert_eq!(v.get("groq").unwrap().as_deref(), Some("gsk_old"));
    }
}

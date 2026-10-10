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
mod session_import;
#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;

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
                // The old keychain item goes too, so it cannot come back via the app's fallback.
                let legacy = self.remove_legacy(&[name]);
                Ok(Reply {
                    note: Some(format!("legacy={}", legacy.as_str())),
                    ..json!({"existed": existed, "legacy": legacy.as_str()}).into()
                })
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

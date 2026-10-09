//! Headless University of Melbourne SSO sign-in.
//!
//! Canvas authenticates through Okta Identity Engine at `sso.unimelb.edu.au`,
//! whose widget is a thin client over a JSON state machine at `/idp/idx/*`. So
//! the flow runs with no webview (see `docs/architecture.md`): introspect the
//! login page's state token, answer each *remediation*, then replay the SAML
//! app URL and POST the `SAMLResponse` to Canvas.
//!
//! Only password and TOTP (Google Authenticator) are answerable; push needs a
//! human. The TOTP seed is shown once, at enrolment, so using this means
//! re-enrolling the factor and copying its setup key.
//!
//! Where the credentials live is the `CredentialStore`'s business. Password
//! and seed sharing one store means that to anything running as this user the
//! second factor is not a second factor — the same deliberate trade as a
//! password manager holding TOTP.
//!
//! The app runs this in-process when keyd is absent; keyd runs it otherwise.
//! The only OS calls are `platform::files`' lock and private writes.

use std::path::{Path, PathBuf};

mod flow;
mod guard;
mod jar;
pub mod totp;
mod wire;

#[cfg(test)]
mod sign_in_tests;

pub use guard::{resume_automatic_sign_in, Trigger};
pub use totp::{base32_decode, totp_at, totp_code, totp_now, totp_seconds_remaining};
pub use wire::{outcome_from_wire, outcome_to_wire, OktaStatus};

use crate::clock::Clock;
use crate::paths;
use crate::platform::Role;

pub const SSO_HOST: &str = "sso.unimelb.edu.au";

/// Never logged, never written outside the store, never sent anywhere but
/// the SSO host.
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub totp_secret: String,
}

const BAD_SEED: &str = "That does not look like a TOTP setup key: it may only contain the letters \
                        A–Z and the digits 2–7.";

/// The one check on credentials before they are saved, wherever they are
/// kept. An undecodable TOTP seed would otherwise surface mid sign-in as an
/// indistinguishable "wrong code". Messages never contain a value, not even one character of it.
pub fn validate_credentials(
    username: &str,
    password: &str,
    totp_secret: &str,
) -> Result<Credentials, String> {
    let username = username.trim();
    let secret = totp_secret.trim().replace(' ', "");
    if username.is_empty() {
        return Err("Username is required.".to_string());
    }
    if password.is_empty() {
        return Err("Password is required.".to_string());
    }
    // The decoder's error names the offending character; a pasted key is a secret.
    base32_decode(&secret).map_err(|_| BAD_SEED.to_string())?;
    Ok(Credentials {
        username: username.to_string(),
        password: password.to_string(),
        totp_secret: secret,
    })
}

/// Where the sign-in finds the saved credentials, and how it forgets a
/// rejected password.
pub trait CredentialStore {
    /// `Ok(None)` when any piece is missing. `Err` when the store refused the
    /// read (a denied prompt, a sandboxed process): the credentials may well
    /// be on file, so it is never "not set up".
    fn load(&self) -> Result<Option<Credentials>, String>;

    /// Drops only the password, keeping username and seed: the response to
    /// `LoginError::BadPassword`, so a stale one is not replayed until Okta
    /// locks the account.
    fn clear_password(&self) -> Result<(), String>;
}

/// Everything the sign-in takes from outside: where its files go, the two
/// origins it talks to, the credentials and the clock. `new` gives production's
/// values; tests point the origins at a loopback server.
pub struct Env<'a> {
    pub data_dir: PathBuf,
    /// Canvas's origin, without a trailing slash.
    pub canvas_base: String,
    /// Okta's origin, without a trailing slash. Its host keys the cookie jar,
    /// so it must differ from Canvas's host.
    pub sso_base: String,
    pub store: &'a dyn CredentialStore,
    /// Unix seconds, for the guard, the log and the TOTP code.
    pub now: Clock,
}

impl<'a> Env<'a> {
    pub fn new(data_dir: &Path, canvas_base: &str, store: &'a dyn CredentialStore) -> Env<'a> {
        Env {
            data_dir: data_dir.to_path_buf(),
            canvas_base: canvas_base.to_string(),
            sso_base: format!("https://{SSO_HOST}"),
            store,
            now: crate::clock::system(),
        }
    }

    pub(crate) fn sso_host(&self) -> String {
        url::Url::parse(&self.sso_base)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_default()
    }
}

/// Why an automated sign-in stopped. Callers act on the variant: a bad
/// password clears the stored one, a network failure keeps the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginError {
    /// No credentials on file — automated sign-in was never set up.
    NotConfigured,
    /// The user signed out; only a sign-in they start lifts it.
    SignedOut,
    /// The keychain refused the read (a denied prompt, a sandboxed process);
    /// the credentials may well be on file. Carries the keychain's error.
    UnreadableCredentials(String),
    BadPassword(String),
    BadTotp(String),
    /// Okta offered only factors we cannot answer; carries their labels.
    UnsupportedFactor(Vec<String>),
    Locked(String),
    Network(String),
    /// The state machine went somewhere this code does not model; carries the
    /// remediation names.
    Unexpected(String),
    /// oculus-keyd refused or failed the request, or the connection broke, so
    /// the sign-in may not have run; carries the client's description of it.
    /// Never a second attempt in-process: that route is for an absent keyd.
    Broker(String),
    /// The attempt guard held a sign-in back; carries seconds until the next
    /// one is allowed.
    Waiting(u64),
    /// The attempt guard refused a sign-in: automatic sign-in stopped after a
    /// failure retrying cannot fix, or the attempt record is unusable. Carries
    /// the reason. A manual sign-in from the app, a sign-in a person finishes
    /// in the browser, or newly saved credentials resume it.
    Paused(String),
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::NotConfigured => write!(
                f,
                "Automated sign-in is not set up — save your username, password and \
                 authenticator setup key first."
            ),
            LoginError::SignedOut => write!(
                f,
                "Signed out, so automatic sign-in is off until you sign in from Settings → \
                 Canvas or run `oculus auth auto`."
            ),
            LoginError::UnreadableCredentials(m) => write!(
                f,
                "The keychain refused to give out the saved sign-in credentials ({m}). They \
                 are not missing — macOS denied access, either to this program or to the key \
                 oculus-keyd opens its vault with."
            ),
            LoginError::BadPassword(m) => write!(f, "Okta rejected the password: {m}"),
            LoginError::BadTotp(m) => write!(
                f,
                "Okta rejected the authenticator code: {m}. If this keeps happening the \
                 stored setup key is for a factor that has since been re-enrolled, or this \
                 Mac's clock has drifted."
            ),
            LoginError::UnsupportedFactor(opts) => write!(
                f,
                "Okta asked for a factor this app cannot answer. It offered: {}. \
                 Automated sign-in needs Google Authenticator (TOTP) enrolled.",
                if opts.is_empty() {
                    "nothing recognisable".to_string()
                } else {
                    opts.join(", ")
                }
            ),
            LoginError::Locked(m) => write!(f, "The account is locked or blocked: {m}"),
            LoginError::Network(m) => write!(f, "Could not reach the sign-in service: {m}"),
            LoginError::Unexpected(m) => write!(f, "Unexpected sign-in step: {m}"),
            LoginError::Broker(m) => write!(f, "The sign-in request to oculus-keyd failed: {m}"),
            LoginError::Waiting(secs) => write!(
                f,
                "Holding off sign-in after the last attempt; try again in {}.",
                if *secs < 60 {
                    format!("{secs} s")
                } else {
                    format!("{} min", secs.div_ceil(60))
                }
            ),
            LoginError::Paused(m) => write!(
                f,
                "Automatic sign-in is paused after: {m}. Sign in from Settings → Canvas, or \
                 save the credentials again with `oculus auth setup`, to resume it."
            ),
        }
    }
}

/// Headless sign-in behind the attempt guard (`guard.rs`). `role` is who asked,
/// and matters only to a manual attempt against a lockout or a rejected
/// password: keyd takes it from its caller, an in-process run states its own.
/// Each attempt is a line in `okta-sign-in.log`.
pub fn sign_in(env: &Env, trigger: Trigger, role: Role) -> Result<String, LoginError> {
    if trigger != Trigger::Manual && paths::signed_out(&env.data_dir).exists() {
        return Err(LoginError::SignedOut);
    }
    let creds = env
        .store
        .load()
        .map_err(LoginError::UnreadableCredentials)?
        .ok_or(LoginError::NotConfigured)?;
    guard::admit_recorded(&env.data_dir, trigger, role, (env.now)())?;

    let result = flow::attempt_sign_in(env, &creds);
    guard::settle_recorded(&env.data_dir, trigger, &result);
    let outcome = match &result {
        Ok(_) => "signed in".to_string(),
        Err(e) => format!("failed — {e}"),
    };
    paths::append_sign_in_log(
        &env.data_dir,
        &format!("{}: {outcome}", trigger.as_str()),
        (env.now)(),
    );
    if let Err(LoginError::BadPassword(_)) = &result {
        // Replaying a wrong password unattended locks the account.
        env.store.clear_password().ok();
    }
    result
}

/// What the sign-in page looks like from here, for when the flow fails.
/// Reports shapes and lengths, never values: a state token is a live
/// credential.
pub fn diagnose(env: &Env) -> String {
    flow::diagnose(env)
}

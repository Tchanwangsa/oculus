//! Headless University of Melbourne SSO sign-in.
//!
//! Canvas authenticates through Okta Identity Engine at `sso.unimelb.edu.au`,
//! whose widget is a thin client over a JSON state machine at `/idp/idx/*`. So
//! the flow runs in Rust with no webview (see `docs/architecture.md`):
//! introspect the login page's state token, answer each *remediation*, then
//! replay the SAML app URL and POST the `SAMLResponse` to Canvas.
//!
//! Only password and TOTP (Google Authenticator) are answerable; push needs a
//! human. The TOTP seed is shown once, at enrolment, so using this means
//! re-enrolling the factor and copying its setup key.
//!
//! Password and seed share the macOS keychain, so to anything running as this
//! user the second factor is not a second factor — the same deliberate trade
//! as a password manager holding TOTP.

mod attempts;
pub(crate) mod commands;
mod diagnose;
mod flow;
mod http;
mod remediation;
mod store;
mod totp;

#[cfg(test)]
mod tests;

pub use attempts::{resume_automatic_sign_in, sign_in, Trigger};
pub use commands::try_auto_recover;
pub use diagnose::diagnose;
pub use http::{LoginError, SSO_HOST};
pub use store::{
    clear_credentials, clear_password, credential_status, store_credentials, CredentialStatus,
    Credentials,
};
pub use totp::{base32_decode, totp_at, totp_now, totp_seconds_remaining};

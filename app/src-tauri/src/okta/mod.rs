//! The app's half of the headless University of Melbourne SSO sign-in.
//!
//! The flow itself (Okta's IDX state machine, the SAML round trip, the
//! attempt guard) is `keyd_core::okta`, and runs only inside `oculus-keyd`:
//! the session it mints goes into keyd's vault, where only keyd can use it.
//! With keyd absent the sign-in fails with `LoginError::Broker`, and only the
//! credential calls (status, save, forget) fall back to the keychain. Any
//! other keyd error surfaces and starts no second route, so a refusal can
//! never become a second attempt against Okta.
//!
//! Password and seed are kept together, so to anything running as this user
//! the second factor is not a second factor — the same deliberate trade as a
//! password manager holding TOTP.

pub(crate) mod commands;
mod credentials;
mod keychain;
mod login;
#[cfg(test)]
mod tests;

pub use credentials::{clear_credentials, credential_status, store_credentials, CredentialStatus};
pub use keyd_core::okta::{totp_now, LoginError, Trigger, SSO_HOST};
pub use login::{diagnose, resume_automatic_sign_in, sign_in, try_auto_recover};

use crate::credentials::Credentialed;

fn broker() -> Credentialed {
    Credentialed::at(&crate::paths::data_dir())
}

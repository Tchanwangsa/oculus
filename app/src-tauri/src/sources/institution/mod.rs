//! Which university this install belongs to: its Canvas address, how its
//! sign-in works, and the per-university details the sync needs.
//!
//! One file, `institution.json` in the data dir, read alike by the app and the
//! CLI. It is not a `settings` row because synchronous code needs the Canvas
//! host without a database pool. The student pastes their Canvas address;
//! everything else is detected (`configure`) or learned from a launch
//! (`learn`). oculus-keyd holds its own copy of the origins it sends the
//! session to (`keyd`). See `docs/auth.md`.

pub mod commands;
mod configure;
mod domain;
mod keyd;
mod profile;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub use configure::{configure, library_exists, normalise};
pub use domain::institution_domain;
pub use keyd::tell_keyd;
pub use profile::{
    canvas_base, canvas_host, current, host_of, is_okta, learn, load, path, save, sso_host,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Institution {
    /// `https://host`, no path or trailing slash.
    pub canvas: String,
    /// Where the sign-in window starts. Canvas sends `/login` to the account's
    /// default provider, unless the university puts a portal page there.
    #[serde(default = "default_login_path")]
    pub login_path: String,
    #[serde(default)]
    pub idp: Idp,
    /// The host the sign-in redirect lands on, when it leaves Canvas.
    #[serde(default)]
    pub sso_host: Option<String>,
    /// A Canvas tool path tried after the course's own tabs
    /// (`sources::echo360`).
    #[serde(default)]
    pub echo360_tool: Option<String>,
    /// Seconds of intro the university puts on every recording, trimmed off.
    #[serde(default)]
    pub echo360_lead_in_secs: u32,
    /// Ed's API base, learned from the first LTI launch (`sources::ed`).
    #[serde(default)]
    pub ed_api: Option<String>,
    /// Course codes that are never subjects (training shells and the like).
    #[serde(default)]
    pub exclude_code_prefixes: Vec<String>,
}

/// Okta Identity Engine is the one IdP Oculus can answer headlessly
/// (`keyd_core::okta`); every other sign-in happens in the visible window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Idp {
    Okta,
    #[default]
    Other,
}

fn default_login_path() -> String {
    "/login".to_string()
}

/// The profile an install that predates this file was built against.
pub fn unimelb() -> Institution {
    Institution {
        canvas: "https://canvas.lms.unimelb.edu.au".to_string(),
        login_path: "/login/saml".to_string(),
        idp: Idp::Okta,
        sso_host: Some("sso.unimelb.edu.au".to_string()),
        echo360_tool: Some("/external_tools/701".to_string()),
        echo360_lead_in_secs: 14,
        ed_api: Some("https://edstem.org/api".to_string()),
        exclude_code_prefixes: vec!["MPMP".to_string()],
    }
}

pub const NOT_CONFIGURED: &str =
    "No Canvas address set — add your university's Canvas address in Settings → Canvas.";

pub const LOCKED: &str = "Your library was synced from this Canvas. Use Start over in \
     Settings → Storage to clear it before changing university.";

/// Ed's origin until a launch lands elsewhere.
const DEFAULT_ED: &str = "https://edstem.org";

impl Institution {
    /// Whether the headless Okta sign-in applies.
    pub fn is_okta(&self) -> bool {
        self.idp == Idp::Okta && self.sso_host.is_some()
    }

    /// Whether the university is in the southern hemisphere, judged by its
    /// Canvas or SSO host's country code. Only term names lean on it
    /// (`sync::terms`).
    pub fn southern_hemisphere(&self) -> bool {
        domain::is_southern(&self.canvas, self.sso_host.as_deref())
    }

    /// The origins oculus-keyd sends the session and the sign-in to. Okta's
    /// only when the IdP is Okta: keyd signs in to nothing else.
    pub fn site(&self) -> keyd_core::site::Site {
        let ed = self
            .ed_api
            .as_deref()
            .and_then(|api| url::Url::parse(api).ok())
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_else(|| DEFAULT_ED.to_string());
        keyd_core::site::Site {
            canvas: self.canvas.clone(),
            login_path: self.login_path.clone(),
            sso: self
                .sso_host
                .as_ref()
                .filter(|_| self.idp == Idp::Okta)
                .map(|host| format!("https://{host}")),
            ed,
        }
    }
}

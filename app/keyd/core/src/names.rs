//! The names secrets are stored under, and the keychain items they came from.

pub const VOYAGE: &str = "voyage";
pub const MINERU: &str = "mineru";
pub const GROQ: &str = "groq";
pub const OKTA_USERNAME: &str = "okta.username";
pub const OKTA_PASSWORD: &str = "okta.password";
pub const OKTA_TOTP_SECRET: &str = "okta.totp_secret";
pub const SESSION_CANVAS: &str = "session.canvas";
pub const SESSION_SSO: &str = "session.sso";
pub const SESSION_ED: &str = "session.ed";

/// The Okta sign-in's three values. They are written only by `okta_save` and
/// `okta_forget`, never by the generic `store` and `delete`.
pub const OKTA: &[&str] = &[OKTA_USERNAME, OKTA_PASSWORD, OKTA_TOTP_SECRET];

/// The login sessions: Canvas's and Okta's cookie headers and Ed's `x-token`.
/// They are written only by the `session_*` ops and by `forward` absorbing a
/// `Set-Cookie`, never by the generic `store` and `delete`.
pub const SESSIONS: &[&str] = &[SESSION_CANVAS, SESSION_SSO, SESSION_ED];

/// Every name the vault accepts; anything else is refused before it is stored.
pub const KNOWN: &[&str] = &[
    VOYAGE,
    MINERU,
    GROQ,
    OKTA_USERNAME,
    OKTA_PASSWORD,
    OKTA_TOTP_SECRET,
    SESSION_CANVAS,
    SESSION_SSO,
    SESSION_ED,
];

pub fn is_known(name: &str) -> bool {
    KNOWN.contains(&name)
}

/// keyd's own bookkeeping entries start with this. None is in `KNOWN`, so no
/// op can read, store or delete one, and none is ever a secret.
pub const KEYD_PREFIX: &str = "keyd.";

/// Present once keyd has copied `name`'s old keychain item into the vault (or
/// found nothing to copy), so a later delete is never undone by a re-import.
pub fn imported(name: &str) -> String {
    format!("{KEYD_PREFIX}imported.{name}")
}

/// A keychain item the app wrote before the vault existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Legacy {
    pub service: &'static str,
    pub account: &'static str,
    pub secret: &'static str,
}

/// `voyage.rs`, `mineru.rs`, `groq.rs` and `okta.rs` in the app own these.
pub const LEGACY: &[Legacy] = &[
    Legacy {
        service: "com.tchan.oculus.voyage",
        account: "voyage",
        secret: VOYAGE,
    },
    Legacy {
        service: "com.tchan.oculus.mineru",
        account: "mineru",
        secret: MINERU,
    },
    Legacy {
        service: "com.tchan.oculus.groq",
        account: "groq",
        secret: GROQ,
    },
    Legacy {
        service: "com.oculus.unimelb-sso",
        account: "username",
        secret: OKTA_USERNAME,
    },
    Legacy {
        service: "com.oculus.unimelb-sso",
        account: "password",
        secret: OKTA_PASSWORD,
    },
    Legacy {
        service: "com.oculus.unimelb-sso",
        account: "totp_secret",
        secret: OKTA_TOTP_SECRET,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_okta_names_are_the_okta_prefix() {
        let by_prefix: Vec<&str> = KNOWN
            .iter()
            .copied()
            .filter(|n| n.starts_with("okta."))
            .collect();
        assert_eq!(by_prefix, OKTA);
    }

    #[test]
    fn the_session_names_are_the_session_prefix() {
        let by_prefix: Vec<&str> = KNOWN
            .iter()
            .copied()
            .filter(|n| n.starts_with("session."))
            .collect();
        assert_eq!(by_prefix, SESSIONS);
    }

    #[test]
    fn every_legacy_item_lands_on_a_known_name_once() {
        let mut seen: Vec<&str> = LEGACY.iter().map(|l| l.secret).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), LEGACY.len());
        assert!(LEGACY.iter().all(|l| is_known(l.secret)));
        assert!(LEGACY.iter().all(|l| !SESSIONS.contains(&l.secret)));
    }

    #[test]
    fn bookkeeping_entries_are_never_known_names() {
        for name in KNOWN {
            assert!(!is_known(&imported(name)));
            assert!(!name.starts_with(KEYD_PREFIX));
        }
    }
}

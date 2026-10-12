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
pub const MUSIC_SPOTIFY: &str = "music.spotify";
pub const WEBCRYPTO_DEV: &str = "webkit.webcrypto.app";
pub const WEBCRYPTO_BUNDLED: &str = "webkit.webcrypto.com.tchan.oculus";

/// The Okta sign-in's three values. They are written only by `okta_save` and
/// `okta_forget`, never by the generic `store` and `delete`.
pub const OKTA: &[&str] = &[OKTA_USERNAME, OKTA_PASSWORD, OKTA_TOTP_SECRET];

/// The login sessions: Canvas's and Okta's cookie headers and Ed's `x-token`.
/// They are written only by the `session_*` ops and by `forward` absorbing a
/// `Set-Cookie`, never by the generic `store` and `delete`.
pub const SESSIONS: &[&str] = &[SESSION_CANVAS, SESSION_SSO, SESSION_ED];

/// WebKit's WebCrypto master keys, one per WebKit application. They are
/// written and read only by `webcrypto_key`, never by `store`, `delete` or
/// `get`, so a key that wraps saved logins cannot be replaced or crossed.
pub const WEBCRYPTO_NAMES: &[&str] = &[WEBCRYPTO_DEV, WEBCRYPTO_BUNDLED];

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
    MUSIC_SPOTIFY,
    WEBCRYPTO_DEV,
    WEBCRYPTO_BUNDLED,
];

/// The names `get` reads back, to the app alone: Spotify's stored
/// credentials, which librespot must hold to log in and which no keyd route
/// could attach for it. Never a cloud key, an Okta value or a session.
pub const READABLE: &[&str] = &[MUSIC_SPOTIFY];

pub fn is_known(name: &str) -> bool {
    KNOWN.contains(&name)
}

/// keyd's own bookkeeping entries start with this. None is in `KNOWN`, so no
/// op can read, store or delete one, and none is ever a secret.
pub const KEYD_PREFIX: &str = "keyd.";

/// The university keyd serves (`crate::site`), as `site_set` stored it.
pub const SITE: &str = "keyd.site";

/// Present once keyd has copied `name`'s old keychain item into the vault (or
/// found nothing to copy), so a later delete is never undone by a re-import.
pub fn imported(name: &str) -> String {
    format!("{KEYD_PREFIX}imported.{name}")
}

/// The account of every WebCrypto master key item WebKit writes: this, then
/// the application's name (`SerializedCryptoKeyWrapMac.mm`).
pub const WEBKIT_ACCOUNT_PREFIX: &str = "com.apple.WebKit.WebCrypto.master+";

/// One WebKit application's WebCrypto master key: the name WebKit files it
/// under (a bundle identifier, else the executable's name), WebKit's own
/// keychain item for it (a generic password with no service), and the vault
/// entry keyd keeps it in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebCrypto {
    pub app: &'static str,
    pub account: &'static str,
    pub secret: &'static str,
}

/// The dev build is the bare executable `app`; the bundle is
/// `paths::IDENTIFIER`. Each is its own WebKit application with its own key,
/// and the two must never be crossed: a wrong key unwraps nothing, silently.
pub const WEBCRYPTO: &[WebCrypto] = &[
    WebCrypto {
        app: "app",
        account: "com.apple.WebKit.WebCrypto.master+app",
        secret: WEBCRYPTO_DEV,
    },
    WebCrypto {
        app: "com.tchan.oculus",
        account: "com.apple.WebKit.WebCrypto.master+com.tchan.oculus",
        secret: WEBCRYPTO_BUNDLED,
    },
];

pub fn webcrypto(app: &str) -> Option<&'static WebCrypto> {
    WEBCRYPTO.iter().find(|w| w.app == app)
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
    fn get_reads_back_only_the_spotify_credentials() {
        assert_eq!(READABLE, [MUSIC_SPOTIFY]);
        for name in READABLE {
            assert!(is_known(name));
            assert!(!OKTA.contains(name) && !SESSIONS.contains(name));
            assert!(!WEBCRYPTO_NAMES.contains(name));
            assert!(LEGACY.iter().all(|l| l.secret != *name));
        }
    }

    #[test]
    fn each_webkit_application_has_its_own_item_and_entry() {
        let secrets: Vec<&str> = WEBCRYPTO.iter().map(|w| w.secret).collect();
        assert_eq!(secrets, WEBCRYPTO_NAMES);
        assert_eq!(webcrypto("app").unwrap().secret, WEBCRYPTO_DEV);
        assert_eq!(
            webcrypto(crate::paths::IDENTIFIER).unwrap().secret,
            WEBCRYPTO_BUNDLED
        );
        for w in WEBCRYPTO {
            assert_eq!(w.account, format!("{WEBKIT_ACCOUNT_PREFIX}{}", w.app));
            assert_eq!(w.secret, format!("webkit.webcrypto.{}", w.app));
            assert!(is_known(w.secret));
            assert!(!READABLE.contains(&w.secret));
            assert!(LEGACY.iter().all(|l| l.secret != w.secret));
        }
        assert_eq!(webcrypto(""), None);
        assert_eq!(webcrypto("App"), None);
        assert_eq!(webcrypto("com.tchan.oculus.keyd"), None);
    }

    #[test]
    fn bookkeeping_entries_are_never_known_names() {
        assert!(SITE.starts_with(KEYD_PREFIX) && !is_known(SITE));
        for name in KNOWN {
            assert!(!is_known(&imported(name)));
            assert!(!name.starts_with(KEYD_PREFIX));
        }
    }
}

//! The three login sessions keyd holds for the Oculus app and CLI: Canvas's
//! cookie header, Okta's cookie header (only the in-app browser replays it)
//! and Ed's `x-token`. Each is one string in the vault. `forward` attaches
//! them to requests for their own origin; only `session_get`, to the app,
//! returns one. The cookie rules (`cookie`) are shared with the sign-in's jar;
//! the vault side (`store`) is keyd's.

pub mod cookie;
#[cfg(feature = "server")]
pub mod store;

use std::sync::atomic::{AtomicU64, Ordering};

use crate::names;

/// Longest session value. A value rides in a request's JSON header line,
/// which is capped at 64 KiB, and JSON escaping can double a value's size.
pub const MAX_VALUE: usize = 24 * 1024;

/// Which session. The wire spells it `canvas`, `sso` or `ed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Canvas,
    Sso,
    Ed,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Canvas, Kind::Sso, Kind::Ed];

    pub fn wire(self) -> &'static str {
        match self {
            Kind::Canvas => "canvas",
            Kind::Sso => "sso",
            Kind::Ed => "ed",
        }
    }

    pub fn parse(text: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.wire() == text)
    }

    /// The vault entry's name.
    pub fn secret(self) -> &'static str {
        match self {
            Kind::Canvas => names::SESSION_CANVAS,
            Kind::Sso => names::SESSION_SSO,
            Kind::Ed => names::SESSION_ED,
        }
    }
}

/// Why `value` cannot be a session: empty, too long for the wire, or holding
/// a character an HTTP header cannot carry. The text never quotes the value.
pub fn check_value(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err("a session value cannot be empty".to_string());
    }
    if value.len() > MAX_VALUE {
        return Err(format!(
            "a session value is at most {MAX_VALUE} bytes; this one is {}",
            value.len()
        ));
    }
    if value.bytes().any(|b| b.is_ascii_control() || b >= 0x80) {
        return Err("a session value has a control or non-ASCII character".to_string());
    }
    Ok(())
}

/// Counts every change to a session (a put, a clear, a `Set-Cookie` that
/// changed the cookie), in memory. A request that read generation `n` and was
/// rejected can tell, by a later reading, whether another caller already
/// replaced the session it sent. Read it before the session, so a change
/// between the two reads looks like more change, never less.
#[derive(Debug, Default)]
pub struct Generation(AtomicU64);

impl Generation {
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    pub fn bump(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_and_name_their_vault_entry() {
        for kind in Kind::ALL {
            assert_eq!(Kind::parse(kind.wire()), Some(kind));
            assert!(names::SESSIONS.contains(&kind.secret()));
        }
        assert_eq!(Kind::parse("Canvas"), None);
        assert_eq!(Kind::parse("session.canvas"), None);
        assert_eq!(Kind::parse(""), None);
    }

    #[test]
    fn a_value_must_be_printable_ascii_and_fit_the_wire() {
        assert!(check_value("canvas_session=abc==; _csrf_token=x%2By").is_ok());
        assert!(check_value(&"a".repeat(MAX_VALUE)).is_ok());
        for bad in [
            String::new(),
            "a".repeat(MAX_VALUE + 1),
            "a=1\r\nHost: evil".to_string(),
            "a=1\u{0}".to_string(),
            "a=\u{fc}".to_string(),
        ] {
            let why = check_value(&bad).unwrap_err();
            assert!(!why.contains("evil"), "{why}");
        }
    }

    #[test]
    fn the_generation_counts_up() {
        let generation = Generation::default();
        assert_eq!(generation.get(), 0);
        generation.bump();
        generation.bump();
        assert_eq!(generation.get(), 2);
    }
}

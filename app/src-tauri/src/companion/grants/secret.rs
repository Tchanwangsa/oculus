//! Share ids and secrets. A share id is 16 random bytes as hex and names the
//! share in URLs; the secret is 32 random bytes as unpadded base64url and
//! travels only in a link's fragment, an `Authorization` header or a WebSocket
//! `hello`. `Secret` has no `Display` and a redacting `Debug`, so it cannot
//! reach a log line or an error string by formatting.

use base64::Engine as _;
use ring::rand::{SecureRandom, SystemRandom};

#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    /// A fresh secret; `None` when the system random source fails, since a
    /// guessable secret is worse than no share.
    pub fn generate() -> Option<Self> {
        let bytes: [u8; 32] = random()?;
        Some(Self(
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes),
        ))
    }

    /// The secret itself, for the share link handed to the desktop only.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Constant-time in the content; the length is public (always 43).
    pub fn matches(&self, candidate: &str) -> bool {
        constant_eq(self.0.as_bytes(), candidate.as_bytes())
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

pub fn share_id() -> Option<String> {
    let bytes: [u8; 16] = random()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn random<const N: usize>() -> Option<[u8; N]> {
    let mut bytes = [0u8; N];
    SystemRandom::new().fill(&mut bytes).ok()?;
    Some(bytes)
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let diff = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
    std::hint::black_box(diff) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_long_random_and_compare_exactly() {
        let a = Secret::generate().unwrap();
        let b = Secret::generate().unwrap();
        assert_eq!(a.expose().len(), 43);
        assert_ne!(a.expose(), b.expose());
        assert!(a.matches(a.expose()));
        assert!(!a.matches(b.expose()));
        assert!(!a.matches(&a.expose()[..42]));
        assert!(!a.matches(""));
        assert_eq!(share_id().unwrap().len(), 32);
    }

    #[test]
    fn debug_never_shows_the_secret() {
        let secret = Secret::generate().unwrap();
        let shown = format!("{secret:?} {secret:#?}");
        assert!(!shown.contains(secret.expose()), "{shown}");
    }
}

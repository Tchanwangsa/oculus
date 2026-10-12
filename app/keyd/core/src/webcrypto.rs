//! WebKit's WebCrypto master key, the key WebKit wraps every `CryptoKey` a
//! page saves to IndexedDB with. WebKit's keychain item holds the padded,
//! standard-alphabet base64 of 16 random bytes: 24 ASCII characters. keyd
//! stores that text and `webcrypto_key` sends it; the app hands WebKit the 16
//! decoded bytes. A key that is anything else is refused, never served,
//! because a wrong key fails silently: IndexedDB answers `undefined`.

use std::fmt;

/// The key's length in bytes.
pub const KEY_LEN: usize = 16;

/// Its length as text: `ceil(16 / 3) * 4`.
pub const TEXT_LEN: usize = 24;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The 16 bytes. Debug prints no byte of them.
#[derive(Clone, PartialEq, Eq)]
pub struct WebCryptoKey([u8; KEY_LEN]);

impl WebCryptoKey {
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        WebCryptoKey(bytes)
    }

    pub fn bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// The text WebKit's item holds and keyd stores.
    pub fn to_text(&self) -> String {
        let mut out = String::with_capacity(TEXT_LEN);
        for chunk in self.0.chunks(3) {
            let n = chunk
                .iter()
                .enumerate()
                .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    /// Exactly the text `to_text` writes: 24 characters, `==` last, and no
    /// stray bits in the final digit. Anything else is `None`.
    pub fn from_text(text: &str) -> Option<Self> {
        let text = text.as_bytes();
        if text.len() != TEXT_LEN || !text.ends_with(b"==") {
            return None;
        }
        let digit = |c: &u8| ALPHABET.iter().position(|a| a == c).map(|d| d as u128);
        let mut bits: u128 = 0;
        for c in &text[..TEXT_LEN - 3] {
            bits = (bits << 6) | digit(c)?;
        }
        // 22 digits carry 132 bits: the key's 128, then 4 that must be zero.
        let last = digit(&text[TEXT_LEN - 3])?;
        if last & 0xf != 0 {
            return None;
        }
        Some(WebCryptoKey(((bits << 2) | (last >> 4)).to_be_bytes()))
    }
}

impl fmt::Debug for WebCryptoKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WebCryptoKey(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_is_standard_padded_base64() {
        // RFC 4648 §10's vectors, carried to 16 bytes.
        let key = WebCryptoKey::from_bytes(*b"foobarfoobarfoob");
        assert_eq!(key.to_text(), "Zm9vYmFyZm9vYmFyZm9vYg==");
        assert_eq!(
            WebCryptoKey::from_text("Zm9vYmFyZm9vYmFyZm9vYg=="),
            Some(key)
        );
        let edges = WebCryptoKey::from_bytes([0xff; 16]);
        assert_eq!(edges.to_text(), "/////////////////////w==");
        let zero = WebCryptoKey::from_bytes([0; 16]);
        assert_eq!(zero.to_text(), "AAAAAAAAAAAAAAAAAAAAAA==");
    }

    #[test]
    fn every_key_survives_the_round_trip() {
        for seed in 0u8..=255 {
            let bytes: [u8; 16] =
                std::array::from_fn(|i| seed.wrapping_mul(31).wrapping_add(i as u8 * 17));
            let key = WebCryptoKey::from_bytes(bytes);
            let text = key.to_text();
            assert_eq!(text.len(), TEXT_LEN);
            assert_eq!(WebCryptoKey::from_text(&text).as_ref(), Some(&key));
        }
    }

    #[test]
    fn anything_but_the_exact_text_of_16_bytes_is_refused() {
        for bad in [
            "",
            "Zm9vYmFyZm9vYmFyZm9vYg=",
            "Zm9vYmFyZm9vYmFyZm9vYg===",
            "Zm9vYmFyZm9vYmFyZm9vYg\n=",
            "Zm9vYmFyZm9vYmFyZm9vYg==\n",
            " Zm9vYmFyZm9vYmFyZm9vYg==",
            "Zm9vYmFyZm9vYmFyZm9vYmFy",
            "Zm9vYmFyZm9vYmFyZm9vYh==",
            "Zm9vYmFyZm9vYmFyZm9v-g==",
            "Zm9vYmFyZm9vYmFyZm9v_g==",
            "Zm9vYmFyZm9vYmFyZm9vY===",
            "Zm9vYmFyZm9vYmFyZm9vYmFyZm9vYmFyZm9vYmFyZm8=",
            "Zm9vYmFyZm9vYmFyZm9v",
        ] {
            assert_eq!(WebCryptoKey::from_text(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn debug_prints_no_byte() {
        let key = WebCryptoKey::from_bytes(*b"foobarfoobarfoob");
        assert_eq!(format!("{key:?}"), "WebCryptoKey(..)");
    }
}

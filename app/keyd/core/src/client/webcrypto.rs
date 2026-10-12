//! `webcrypto_key`: WebKit's WebCrypto master key for the WebKit application
//! the caller is, which the app hands to WebKit in place of WebKit's own
//! keychain read (`crate::webcrypto`).

use serde_json::{json, Value};

use super::{Client, KeydError, OP_TIMEOUT};
use crate::webcrypto::WebCryptoKey;

/// A key keyd served, and where keyd found it: `vault`, `keychain` (WebKit's
/// item, copied in just now) or `minted` (WebKit had none). Debug prints no
/// byte of the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedKey {
    pub key: WebCryptoKey,
    pub source: String,
}

impl Client {
    /// The key for `app`, the name WebKit files this process's key under: its
    /// bundle identifier, or else its executable's name (`names::WEBCRYPTO`).
    /// App only, and only the application the caller is: anything else is
    /// `KeydError::Caller`. `KeydError::Keychain` means WebKit's item could
    /// not be read and nothing was stored; like `Absent`, the caller then
    /// leaves the key to WebKit. A reply that is not a 16-byte key is
    /// `Broken`, never a key.
    pub fn webcrypto_key(&self, app: &str) -> Result<ServedKey, KeydError> {
        let (reply, _) = self.exchange(
            &json!({"op": "webcrypto_key", "app": app}),
            &[],
            Some(OP_TIMEOUT),
        )?;
        let key = reply
            .get("key")
            .and_then(Value::as_str)
            .and_then(WebCryptoKey::from_text)
            .ok_or_else(|| {
                KeydError::Broken("webcrypto_key: no 16-byte key in the reply".into())
            })?;
        let source = reply
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        Ok(ServedKey { key, source })
    }
}

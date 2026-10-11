//! `webcrypto_key`: WebKit's WebCrypto master key (`crate::webcrypto`), to
//! the app alone, so WebKit stops reading its own keychain item, whose access
//! prompt returns with every rebuild of an ad-hoc signed app.
//!
//! The dev build and the bundle are two WebKit applications with two keys,
//! and both use this one vault. A request names its application, and keyd
//! serves it only when that is the application the peer check found the
//! caller to be (`Caller::application`), so neither build can get the other's.
//!
//! The first request for an application copies WebKit's item into the vault.
//! Only a keychain that says there is no item leads to a new key: a refused
//! or failed read stores nothing, and the app lets WebKit read its item itself.

use std::sync::PoisonError;

use serde_json::{json, Value};

use super::{OpError, Reply, State};
use crate::names::{self, WebCrypto};
use crate::platform::Caller;
use crate::vault::KeyError;
use crate::webcrypto::{WebCryptoKey, KEY_LEN};

/// Where a served key came from, for the reply and the log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Vault,
    /// Copied from WebKit's keychain item just now.
    Keychain,
    /// Made just now: the keychain had no item.
    Minted,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Source::Vault => "vault",
            Source::Keychain => "keychain",
            Source::Minted => "minted",
        }
    }
}

impl State {
    /// `{"app"}` to `{"key": text, "source": "vault"|"keychain"|"minted"}`.
    pub(super) fn webcrypto_key(&self, caller: &Caller, req: &Value) -> Result<Reply, OpError> {
        let app = req
            .get("app")
            .and_then(Value::as_str)
            .ok_or_else(|| OpError::new("request", "webcrypto_key needs a string \"app\""))?;
        let item = names::webcrypto(app)
            .ok_or_else(|| OpError::new("request", format!("no WebCrypto key for {app:?}")))?;
        if caller.application.as_deref() != Some(item.app) {
            return Err(OpError::new(
                "caller",
                format!("this program is not the WebKit application {app:?}"),
            ));
        }
        let (key, source) = self.webcrypto_once(item)?;
        Ok(Reply {
            note: Some(format!("app={app} source={}", source.as_str())),
            ..json!({"key": key.to_text(), "source": source.as_str()}).into()
        })
    }

    /// The vault's key for `item`, or WebKit's copied in, or a new one when
    /// WebKit has none. One at a time (`importing`), so racing first requests
    /// read the item once and get the same key; the vault keeps whichever
    /// key it held first.
    fn webcrypto_once(&self, item: &WebCrypto) -> Result<(WebCryptoKey, Source), OpError> {
        let vault = self.vault()?;
        let _one = self
            .importing
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(text) = vault.get(item.secret)? {
            return Ok((stored(&text)?, Source::Vault));
        }
        // May prompt, so it runs outside the vault's lock.
        let (key, source) = match self.legacy.read_webcrypto(item) {
            Ok(Some(data)) => (from_item(item, &data)?, Source::Keychain),
            Ok(None) => (mint()?, Source::Minted),
            Err(KeyError::Refused(d) | KeyError::Platform(d)) => {
                return Err(OpError::new("keychain", d))
            }
        };
        let text = key.to_text();
        let kept = vault.update(|e| match e.get(item.secret) {
            Some(held) => held.to_string(),
            None => {
                e.insert(item.secret, &text);
                text.clone()
            }
        })?;
        if kept != text {
            return Ok((stored(&kept)?, Source::Vault));
        }
        Ok((key, source))
    }
}

/// A vault entry that is not a key is never served, and never replaced.
fn stored(text: &str) -> Result<WebCryptoKey, OpError> {
    WebCryptoKey::from_text(text).ok_or_else(|| {
        OpError::new(
            "vault",
            "the stored WebCrypto key is not the base64 of 16 bytes",
        )
    })
}

/// WebKit's item holds the key's text as UTF-8; nothing else is copied.
fn from_item(item: &WebCrypto, data: &[u8]) -> Result<WebCryptoKey, OpError> {
    std::str::from_utf8(data)
        .ok()
        .and_then(WebCryptoKey::from_text)
        .ok_or_else(|| {
            OpError::new(
                "keychain",
                format!(
                    "{} does not hold the base64 of 16 bytes ({} bytes)",
                    item.account,
                    data.len()
                ),
            )
        })
}

fn mint() -> Result<WebCryptoKey, OpError> {
    let mut bytes = [0u8; KEY_LEN];
    getrandom::getrandom(&mut bytes)
        .map_err(|e| OpError::new("keychain", format!("getrandom: {e}")))?;
    Ok(WebCryptoKey::from_bytes(bytes))
}

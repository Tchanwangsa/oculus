//! Keychain storage shared by cloud keys and university sign-in credentials,
//! and `Credentialed`, the client of `oculus-keyd` (`keyd_core::client`).
//! Callers own validation and probing; values never leave the keychain here.

pub(crate) use keyd_core::client::{Client as Credentialed, KeydError, RawResponse};

pub(crate) struct Secret<'a> {
    service: &'a str,
    account: &'a str,
}

impl<'a> Secret<'a> {
    pub(crate) const fn new(service: &'a str, account: &'a str) -> Self {
        Self { service, account }
    }

    fn entry(&self) -> Result<keyring::Entry, String> {
        keyring::Entry::new(self.service, self.account).map_err(|e| e.to_string())
    }

    /// A refusal reads as no value. Use `fetch` or `has` wherever the caller
    /// can report the refusal instead.
    pub(crate) fn read(&self) -> Option<String> {
        self.fetch().ok().flatten()
    }

    /// Whether a value is saved, for Settings. A refusal is an `Err` naming
    /// `what`, so a page never offers to replace a key that is still there.
    pub(crate) fn has(&self, what: &str) -> Result<bool, String> {
        self.fetch()
            .map(|value| value.is_some())
            .map_err(|e| format!("The keychain refused to give out the {what} ({e})"))
    }

    /// `Ok(None)` only when no item exists. A refused prompt or a sandboxed
    /// process is an `Err`, so a saved key is never reported as missing.
    pub(crate) fn fetch(&self) -> Result<Option<String>, String> {
        match self.entry()?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    pub(crate) fn write(&self, value: &str) -> Result<(), String> {
        self.entry()?.set_password(value).map_err(|e| e.to_string())
    }

    pub(crate) fn delete(&self) -> Result<(), String> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    /// `store_checked` into this keychain item.
    pub(crate) fn store_checked(
        &self,
        key: &str,
        probe: impl FnOnce(&str) -> Result<Verdict, String>,
    ) -> Result<String, String> {
        store_checked(key, probe, |value| self.write(value))
    }
}

/// Probe before storing. An unreachable service can accept a key on trust;
/// an active refusal leaves the saved key untouched. `write` stores the
/// trimmed key: into the keychain, or through keyd.
pub(crate) fn store_checked(
    key: &str,
    probe: impl FnOnce(&str) -> Result<Verdict, String>,
    write: impl FnOnce(&str) -> Result<(), String>,
) -> Result<String, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("empty key".into());
    }
    let verdict = probe(key)?;
    write(key)?;
    Ok(match verdict {
        Verdict::Good => "ok".into(),
        Verdict::Unverified => "unverified".into(),
    })
}

/// What a probe learned about a key. `Unverified`: the service was unreachable,
/// or answered about the account rather than the key.
pub(crate) enum Verdict {
    Good,
    Unverified,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_keys_are_rejected_before_probing_or_touching_the_keychain() {
        let key = Secret::new("test-unused", "test-unused");
        assert_eq!(key.store_checked(" \n\t ", |_| panic!("must not probe")).unwrap_err(), "empty key");
    }

    #[test]
    fn probes_receive_trimmed_keys_and_refusal_prevents_storage() {
        let key = Secret::new("test-unused", "test-unused");
        let result = key.store_checked("  pa-key \n", |value| {
            assert_eq!(value, "pa-key");
            Err("refused".into())
        });
        assert_eq!(result.unwrap_err(), "refused");
    }
}

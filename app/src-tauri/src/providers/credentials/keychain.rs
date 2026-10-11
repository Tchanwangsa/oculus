//! The old keychain items, read and written through `keyring`: the fallback
//! for every credential while keyd is absent, removed with that fallback. The
//! only place the app names the OS keychain (`keyd-seams.test.mjs` allowlists
//! this file).

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
}

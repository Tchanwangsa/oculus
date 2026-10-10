use super::{KeyError, MasterKey};

/// Where the master key comes from: the OS secret store in production
/// (`platform::master_key`), a fixed key in tests.
pub trait KeySource: Send + Sync {
    /// Reads the key, creating it on first use. Never replaces one that exists.
    fn get_or_create(&self) -> Result<MasterKey, KeyError>;
}

/// A key held in memory, for tests and test hooks.
pub struct StaticKey(pub MasterKey);

impl KeySource for StaticKey {
    fn get_or_create(&self) -> Result<MasterKey, KeyError> {
        Ok(self.0.clone())
    }
}

/// Reads and deletes the items that predate the vault (`names::LEGACY`): the
/// OS secret store in production (`platform::legacy_items`), a map in tests.
pub trait LegacySource: Send + Sync {
    /// `Ok(None)` only when no such item exists.
    fn read(&self, service: &str, account: &str) -> Result<Option<String>, KeyError>;

    /// Deletes one item: `Ok(true)` when it was there, `Ok(false)` when no
    /// such item exists.
    fn remove(&self, service: &str, account: &str) -> Result<bool, KeyError>;
}

/// No old items: for a keyd whose master key is not the store's either.
#[cfg(debug_assertions)]
pub struct NoLegacy;

#[cfg(debug_assertions)]
impl LegacySource for NoLegacy {
    fn read(&self, _service: &str, _account: &str) -> Result<Option<String>, KeyError> {
        Ok(None)
    }

    fn remove(&self, _service: &str, _account: &str) -> Result<bool, KeyError> {
        Ok(false)
    }
}

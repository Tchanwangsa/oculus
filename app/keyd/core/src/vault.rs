//! The secret store behind `oculus-keyd`: every secret as one AEAD-sealed JSON
//! map in `<data_dir>/vault.bin`, under one 256-bit master key that lives in
//! the OS secret store (`platform::master_key`). See docs/architecture.md.
//!
//! `vault.bin` is a version byte, a 12-byte nonce, then the ChaCha20-Poly1305
//! ciphertext (tag included) of a JSON object of strings. The version byte is
//! the associated data. Every write draws a fresh nonce and replaces the file
//! by rename; read-modify-write runs under an exclusive lock on
//! `vault.bin.lock`.
//!
//! No value is ever logged or Debug-printed: `Entries` and `MasterKey` print
//! names and nothing else.

mod entries;
mod master_key;
mod sealed;
mod sources;
mod store;
#[cfg(test)]
mod tests;

pub use entries::Entries;
pub use master_key::{KeyError, MasterKey, MASTER_ACCOUNT, MASTER_LABEL, MASTER_SERVICE};
#[cfg(debug_assertions)]
pub use sources::NoLegacy;
pub use sources::{KeySource, LegacySource, StaticKey};
pub use store::{Vault, VaultError};

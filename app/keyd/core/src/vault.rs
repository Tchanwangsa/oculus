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

use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

use crate::platform::files;

const VERSION: u8 = 1;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

/// The master key's item in the OS secret store. The label is what an
/// access prompt quotes.
pub const MASTER_SERVICE: &str = "com.tchan.oculus.keyd";
pub const MASTER_ACCOUNT: &str = "master";
pub const MASTER_LABEL: &str = "Oculus keys";

// ── Master key ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct MasterKey([u8; 32]);

impl MasterKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        MasterKey(bytes)
    }

    pub fn generate() -> Result<Self, KeyError> {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).map_err(|e| KeyError::Platform(format!("getrandom: {e}")))?;
        Ok(MasterKey(bytes))
    }

    /// The secret store holds the key as 64 lowercase hex characters.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn from_hex(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.len() != 64 || !text.is_ascii() {
            return None;
        }
        let mut bytes = [0u8; 32];
        for (i, out) in bytes.iter_mut().enumerate() {
            *out = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
        }
        Some(MasterKey(bytes))
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterKey(..)")
    }
}

/// Why the master key, or an old item, could not be had. `Refused` is the
/// user or the sandbox saying no (a cancelled prompt, no access); `Platform`
/// is anything else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    Refused(String),
    Platform(String),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::Refused(d) => write!(f, "the keychain refused the master key: {d}"),
            KeyError::Platform(d) => write!(f, "the keychain failed: {d}"),
        }
    }
}

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

/// Reads one item that predates the vault (`names::LEGACY`): the OS secret
/// store in production (`platform::legacy_items`), a map in tests.
/// `Ok(None)` only when no such item exists.
pub trait LegacySource: Send + Sync {
    fn read(&self, service: &str, account: &str) -> Result<Option<String>, KeyError>;
}

/// No old items: for a keyd whose master key is not the store's either.
#[cfg(debug_assertions)]
pub struct NoLegacy;

#[cfg(debug_assertions)]
impl LegacySource for NoLegacy {
    fn read(&self, _service: &str, _account: &str) -> Result<Option<String>, KeyError> {
        Ok(None)
    }
}

// ── Entries ──────────────────────────────────────────────────────────────────

/// The decrypted map. Debug lists the names only.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Entries(BTreeMap<String, String>);

impl Entries {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    pub fn insert(&mut self, name: &str, value: &str) {
        self.0.insert(name.to_string(), value.to_string());
    }

    pub fn remove(&mut self, name: &str) -> bool {
        self.0.remove(name).is_some()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Entries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

// ── Vault ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultError {
    Io(String),
    /// The file is not a vault this build can read: truncated, an unknown
    /// version, or a decrypted body that is not a map of strings.
    Damaged(String),
    /// Authentication failed: the wrong master key, or altered bytes.
    Undecryptable,
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VaultError::Io(d) => write!(f, "vault.bin: {d}"),
            VaultError::Damaged(d) => write!(f, "vault.bin is damaged: {d}"),
            VaultError::Undecryptable => {
                f.write_str("vault.bin does not decrypt with the master key (a different key, or altered bytes)")
            }
        }
    }
}

pub struct Vault {
    path: PathBuf,
    key: MasterKey,
}

impl fmt::Debug for Vault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vault").field("path", &self.path).finish_non_exhaustive()
    }
}

impl Vault {
    pub fn new(path: impl Into<PathBuf>, key: MasterKey) -> Self {
        Vault { path: path.into(), key }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Everything in the vault. A missing file is an empty vault.
    pub fn load(&self) -> Result<Entries, VaultError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => open_sealed(&self.key, &bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Entries::default()),
            Err(e) => Err(VaultError::Io(format!("reading {}: {e}", self.path.display()))),
        }
    }

    pub fn has(&self, name: &str) -> Result<bool, VaultError> {
        Ok(self.load()?.contains(name))
    }

    pub fn get(&self, name: &str) -> Result<Option<String>, VaultError> {
        Ok(self.load()?.get(name).map(str::to_string))
    }

    pub fn store(&self, name: &str, value: &str) -> Result<(), VaultError> {
        self.update(|e| e.insert(name, value))
    }

    /// True when `name` was there.
    pub fn remove(&self, name: &str) -> Result<bool, VaultError> {
        self.update(|e| e.remove(name))
    }

    /// Read-modify-write under the lock. The file is rewritten only when `f`
    /// changed something, and never when the current file fails to open — a
    /// wrong key must not replace a vault it cannot read.
    pub fn update<R>(&self, f: impl FnOnce(&mut Entries) -> R) -> Result<R, VaultError> {
        let _lock = self.lock()?;
        let before = self.load()?;
        let mut entries = before.clone();
        let out = f(&mut entries);
        if entries != before {
            self.replace(&seal(&self.key, &entries)?)?;
        }
        Ok(out)
    }

    fn lock(&self) -> Result<files::FileLock, VaultError> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| VaultError::Io(format!("creating {}: {e}", dir.display())))?;
        }
        let mut name = self.path.clone().into_os_string();
        name.push(".lock");
        let lock_path = PathBuf::from(name);
        // Blocks until the holder closes its descriptor; released on drop.
        files::lock(&lock_path).map_err(|e| VaultError::Io(format!("locking {}: {e}", lock_path.display())))
    }

    /// Writes a temp file beside the vault and renames it over: a reader sees
    /// the old file or the new one, never part of either.
    fn replace(&self, bytes: &[u8]) -> Result<(), VaultError> {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        let mut suffix = [0u8; 6];
        getrandom::getrandom(&mut suffix).map_err(|e| VaultError::Io(format!("getrandom: {e}")))?;
        let suffix: String = suffix.iter().map(|b| format!("{b:02x}")).collect();
        let file_name = self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let tmp = dir.join(format!(".{file_name}.{}.{suffix}.tmp", std::process::id()));

        let written = (|| {
            let mut f = files::create_private(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
            std::fs::rename(&tmp, &self.path)
        })();
        if let Err(e) = written {
            std::fs::remove_file(&tmp).ok();
            return Err(VaultError::Io(format!("writing {}: {e}", self.path.display())));
        }
        // The rename is durable once the directory is flushed.
        files::sync_dir(dir).ok();
        Ok(())
    }
}

fn cipher(key: &MasterKey) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(Key::from_slice(&key.0))
}

fn seal(key: &MasterKey, entries: &Entries) -> Result<Vec<u8>, VaultError> {
    let plain = serde_json::to_vec(&entries.0).map_err(|e| VaultError::Io(format!("encoding: {e}")))?;
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce).map_err(|e| VaultError::Io(format!("getrandom: {e}")))?;
    let sealed = cipher(key)
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: &plain, aad: &[VERSION] })
        .map_err(|_| VaultError::Io("encryption failed".to_string()))?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + sealed.len());
    out.push(VERSION);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

fn open_sealed(key: &MasterKey, bytes: &[u8]) -> Result<Entries, VaultError> {
    if bytes.len() < 1 + NONCE_LEN + TAG_LEN {
        return Err(VaultError::Damaged(format!("{} bytes is shorter than any vault", bytes.len())));
    }
    if bytes[0] != VERSION {
        return Err(VaultError::Damaged(format!("unknown format version {}", bytes[0])));
    }
    let (nonce, sealed) = bytes[1..].split_at(NONCE_LEN);
    let plain = cipher(key)
        .decrypt(Nonce::from_slice(nonce), Payload { msg: sealed, aad: &[VERSION] })
        .map_err(|_| VaultError::Undecryptable)?;
    serde_json::from_slice::<BTreeMap<String, String>>(&plain)
        .map(Entries)
        .map_err(|_| VaultError::Damaged("the decrypted body is not a map of strings".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names;
    use crate::paths::vault as path_in;
    use std::fs::File;
    use std::io::Read;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let mut r = [0u8; 4];
            getrandom::getrandom(&mut r).unwrap();
            let dir = std::env::temp_dir().join(format!(
                "keyd-vault-{name}-{}-{:08x}",
                std::process::id(),
                u32::from_ne_bytes(r)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn key(byte: u8) -> MasterKey {
        MasterKey::from_bytes([byte; 32])
    }

    #[test]
    fn values_round_trip_through_a_fresh_handle() {
        let dir = Scratch::new("round-trip");
        let path = path_in(&dir.0);
        let v = Vault::new(&path, key(1));
        assert!(v.load().unwrap().is_empty(), "a missing file is an empty vault");
        v.store(names::VOYAGE, "pa-123").unwrap();
        v.store(names::OKTA_PASSWORD, "hunter2 ünïcode \"quoted\"\n").unwrap();

        let again = Vault::new(&path, key(1));
        assert_eq!(again.get(names::VOYAGE).unwrap().as_deref(), Some("pa-123"));
        assert_eq!(again.get(names::OKTA_PASSWORD).unwrap().as_deref(), Some("hunter2 ünïcode \"quoted\"\n"));
        assert!(again.has(names::VOYAGE).unwrap());
        assert!(!again.has(names::GROQ).unwrap());

        assert!(again.remove(names::VOYAGE).unwrap());
        assert!(!again.remove(names::VOYAGE).unwrap());
        assert!(!v.has(names::VOYAGE).unwrap());
        assert!(v.has(names::OKTA_PASSWORD).unwrap());
    }

    #[test]
    fn the_wrong_key_fails_and_never_overwrites() {
        let dir = Scratch::new("wrong-key");
        let path = path_in(&dir.0);
        Vault::new(&path, key(1)).store(names::GROQ, "gsk-1").unwrap();
        let before = std::fs::read(&path).unwrap();

        let wrong = Vault::new(&path, key(2));
        assert_eq!(wrong.load().unwrap_err(), VaultError::Undecryptable);
        assert_eq!(wrong.has(names::GROQ).unwrap_err(), VaultError::Undecryptable);
        assert_eq!(wrong.store(names::MINERU, "x").unwrap_err(), VaultError::Undecryptable);
        assert_eq!(std::fs::read(&path).unwrap(), before, "a failed open must leave the file alone");
        assert_eq!(Vault::new(&path, key(1)).get(names::GROQ).unwrap().as_deref(), Some("gsk-1"));
    }

    #[test]
    fn truncated_or_altered_files_fail_cleanly() {
        let dir = Scratch::new("damaged");
        let path = path_in(&dir.0);
        let v = Vault::new(&path, key(3));
        v.store(names::MINERU, "token").unwrap();
        let good = std::fs::read(&path).unwrap();

        for len in [0, 1, 12, 1 + NONCE_LEN + TAG_LEN - 1] {
            std::fs::write(&path, &good[..len]).unwrap();
            assert!(matches!(v.load(), Err(VaultError::Damaged(_))), "truncated to {len}");
        }
        // Long enough to parse, short of the real ciphertext.
        std::fs::write(&path, &good[..good.len() - 1]).unwrap();
        assert_eq!(v.load().unwrap_err(), VaultError::Undecryptable);

        let mut flipped = good.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 0x01;
        std::fs::write(&path, &flipped).unwrap();
        assert_eq!(v.load().unwrap_err(), VaultError::Undecryptable);

        let mut nonce = good.clone();
        nonce[3] ^= 0x80;
        std::fs::write(&path, &nonce).unwrap();
        assert_eq!(v.load().unwrap_err(), VaultError::Undecryptable);

        let mut version = good.clone();
        version[0] = 9;
        std::fs::write(&path, &version).unwrap();
        assert!(matches!(v.load(), Err(VaultError::Damaged(_))));

        // Sealed correctly, but not a map of strings.
        let plain = br#"{"voyage": 5}"#;
        let n = [7u8; NONCE_LEN];
        let sealed = cipher(&key(3)).encrypt(Nonce::from_slice(&n), Payload { msg: plain, aad: &[VERSION] }).unwrap();
        let mut odd = vec![VERSION];
        odd.extend_from_slice(&n);
        odd.extend_from_slice(&sealed);
        std::fs::write(&path, &odd).unwrap();
        assert!(matches!(v.load(), Err(VaultError::Damaged(_))));
    }

    #[test]
    fn a_write_replaces_the_file_by_rename() {
        let dir = Scratch::new("atomic");
        let path = path_in(&dir.0);
        let v = Vault::new(&path, key(4));
        v.store(names::VOYAGE, "old").unwrap();

        // A reader that opened the old file keeps reading the whole old file.
        let mut held = File::open(&path).unwrap();
        v.store(names::VOYAGE, "new").unwrap();
        let mut old = Vec::new();
        held.read_to_end(&mut old).unwrap();
        assert_eq!(open_sealed(&key(4), &old).unwrap().get(names::VOYAGE), Some("old"));
        assert_eq!(v.get(names::VOYAGE).unwrap().as_deref(), Some("new"));

        let mut leftovers: Vec<String> = std::fs::read_dir(&dir.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        leftovers.sort();
        assert_eq!(leftovers, ["vault.bin", "vault.bin.lock"], "no temp file left behind");
        // Its mode is the platform's to test (`platform/unix.rs`).
    }

    #[test]
    fn every_write_draws_a_fresh_nonce() {
        let dir = Scratch::new("nonce");
        let path = path_in(&dir.0);
        let v = Vault::new(&path, key(5));
        v.store(names::GROQ, "a").unwrap();
        let first = std::fs::read(&path).unwrap();
        v.store(names::GROQ, "b").unwrap();
        v.store(names::GROQ, "a").unwrap();
        let third = std::fs::read(&path).unwrap();
        assert_ne!(first[1..1 + NONCE_LEN], third[1..1 + NONCE_LEN]);
        assert_ne!(first, third);
    }

    #[test]
    fn an_unchanged_update_does_not_rewrite() {
        let dir = Scratch::new("unchanged");
        let path = path_in(&dir.0);
        let v = Vault::new(&path, key(6));
        v.store(names::GROQ, "a").unwrap();
        let before = std::fs::read(&path).unwrap();
        v.store(names::GROQ, "a").unwrap();
        assert!(!v.remove(names::MINERU).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn concurrent_writers_lose_nothing() {
        let dir = Scratch::new("concurrent");
        let path = path_in(&dir.0);
        let threads: Vec<_> = (0..8)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    Vault::new(path, key(7)).store(&format!("t{i}"), "v").unwrap();
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        let entries = Vault::new(&path, key(7)).load().unwrap();
        assert_eq!(entries.names().count(), 8);
    }

    #[test]
    fn debug_output_never_carries_a_value() {
        let mut e = Entries::default();
        e.insert(names::VOYAGE, "pa-secret-value");
        let k = MasterKey::from_hex(&"ab".repeat(32)).unwrap();
        let shown = format!("{e:?} {k:?} {:?}", Vault::new("/x/vault.bin", k.clone()));
        assert!(shown.contains("voyage"));
        assert!(!shown.contains("pa-secret-value"));
        assert!(!shown.contains("abab"));
    }

    #[test]
    fn the_master_key_hex_round_trips_and_rejects_junk() {
        let k = MasterKey::generate().unwrap();
        let hex = k.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(MasterKey::from_hex(&hex).unwrap().0, k.0);
        assert_eq!(MasterKey::from_hex(&format!("{hex}\n")).unwrap().0, k.0);
        assert!(MasterKey::from_hex(&hex[..62]).is_none());
        assert!(MasterKey::from_hex(&"zz".repeat(32)).is_none());
        assert!(MasterKey::from_hex(&"é".repeat(32)).is_none());
    }
}

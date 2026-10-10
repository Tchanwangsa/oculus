use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::sealed::{open_sealed, seal};
use super::{Entries, MasterKey};
use crate::platform::files;

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
        f.debug_struct("Vault")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Vault {
    pub fn new(path: impl Into<PathBuf>, key: MasterKey) -> Self {
        Vault {
            path: path.into(),
            key,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Everything in the vault. A missing file is an empty vault.
    pub fn load(&self) -> Result<Entries, VaultError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => open_sealed(&self.key, &bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Entries::default()),
            Err(e) => Err(VaultError::Io(format!(
                "reading {}: {e}",
                self.path.display()
            ))),
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
            std::fs::create_dir_all(dir)
                .map_err(|e| VaultError::Io(format!("creating {}: {e}", dir.display())))?;
        }
        let mut name = self.path.clone().into_os_string();
        name.push(".lock");
        let lock_path = PathBuf::from(name);
        // Blocks until the holder closes its descriptor; released on drop.
        files::lock(&lock_path)
            .map_err(|e| VaultError::Io(format!("locking {}: {e}", lock_path.display())))
    }

    /// Writes a temp file beside the vault and renames it over: a reader sees
    /// the old file or the new one, never part of either.
    fn replace(&self, bytes: &[u8]) -> Result<(), VaultError> {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        let mut suffix = [0u8; 6];
        getrandom::getrandom(&mut suffix).map_err(|e| VaultError::Io(format!("getrandom: {e}")))?;
        let suffix: String = suffix.iter().map(|b| format!("{b:02x}")).collect();
        let file_name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let tmp = dir.join(format!(".{file_name}.{}.{suffix}.tmp", std::process::id()));

        let written = (|| {
            let mut f = files::create_private(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
            std::fs::rename(&tmp, &self.path)
        })();
        if let Err(e) = written {
            std::fs::remove_file(&tmp).ok();
            return Err(VaultError::Io(format!(
                "writing {}: {e}",
                self.path.display()
            )));
        }
        // The rename is durable once the directory is flushed.
        files::sync_dir(dir).ok();
        Ok(())
    }
}

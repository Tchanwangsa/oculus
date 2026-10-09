//! The file helpers on any POSIX system: the vault's lock and owner-only
//! files, the durable rename an install uses. Shared by every Unix adapter.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Held while the lock is; dropping it closes the descriptor and releases it.
pub struct FileLock(#[allow(dead_code)] File);

/// An exclusive `flock` on `path`, created owner-only if missing. Blocks
/// until the holder lets go.
pub fn lock(path: &Path) -> io::Result<FileLock> {
    let file = OpenOptions::new().create(true).truncate(false).write(true).mode(0o600).open(path)?;
    loop {
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
            return Ok(FileLock(file));
        }
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// A new file only this user can read, failing if `path` exists.
pub fn create_private(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)
}

/// Flushes `dir`, so a rename inside it survives a crash.
pub fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

pub fn set_executable(path: &Path) -> io::Result<()> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

/// Writes through `fill` into a sibling temp file, then renames it over
/// `dest`: overwriting a signed binary in place leaves the kernel's cached
/// signature stale, and a reader never sees half a file.
pub fn replace_file(dest: &Path, fill: impl FnOnce(&Path) -> io::Result<()>) -> Result<(), String> {
    let dir = dest.parent().ok_or_else(|| format!("{} has no parent", dest.display()))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let done = fill(&tmp).and_then(|()| std::fs::rename(&tmp, dest));
    if let Err(e) = done {
        std::fs::remove_file(&tmp).ok();
        return Err(format!("writing {}: {e}", dest.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(std::path::PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn scratch(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("keyd-unix-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn replace_file_leaves_no_temp_behind() {
        let dir = scratch("replace");
        let dest = dir.0.join("sub/stamp");
        replace_file(&dest, |tmp| std::fs::write(tmp, "a\n")).unwrap();
        replace_file(&dest, |tmp| std::fs::write(tmp, "b\n")).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "b\n");
        assert!(replace_file(&dest, |_| Err(io::Error::other("no"))).is_err());
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "b\n");
        assert_eq!(std::fs::read_dir(dir.0.join("sub")).unwrap().count(), 1);
    }

    #[test]
    fn private_files_and_locks_are_owner_only_and_executables_are_not() {
        let dir = scratch("modes");
        let f = dir.0.join("f");
        create_private(&f).unwrap();
        assert_eq!(mode(&f), 0o600);
        assert!(create_private(&f).is_err(), "never over an existing file");
        let l = dir.0.join("l.lock");
        drop(lock(&l).unwrap());
        assert_eq!(mode(&l), 0o600);
        set_executable(&f).unwrap();
        assert_eq!(mode(&f), 0o755);
        sync_dir(&dir.0).unwrap();
    }

    /// The vault's sealed file is written through `create_private`.
    #[cfg(feature = "server")]
    #[test]
    fn the_vault_file_is_owner_only() {
        let dir = scratch("vault-mode");
        let path = crate::paths::vault(&dir.0);
        crate::vault::Vault::new(&path, crate::vault::MasterKey::from_bytes([4; 32])).store("voyage", "x").unwrap();
        assert_eq!(mode(&path), 0o600);
    }
}

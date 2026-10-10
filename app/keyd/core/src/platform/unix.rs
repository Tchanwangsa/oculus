//! The file helpers on any POSIX system: the vault's lock and owner-only
//! files, the durable renames an install uses. Shared by every Unix adapter.

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
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(path)?;
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
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

/// Writes `body` to `path` readable by this user only. A new file is created
/// 0600 and an existing one narrowed before the body lands, so the secret is
/// never on disk world-readable.
pub fn write_private(path: &Path, body: &[u8]) -> io::Result<()> {
    use std::io::Write;

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    file.write_all(body)
}

/// Whether only this user can read or write `path`: no group or other bit.
pub fn is_owner_only(path: &Path) -> io::Result<bool> {
    Ok(std::fs::metadata(path)?.permissions().mode() & 0o077 == 0)
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
    let dir = dest
        .parent()
        .ok_or_else(|| format!("{} has no parent", dest.display()))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let done = fill(&tmp).and_then(|()| std::fs::rename(&tmp, dest));
    if let Err(e) = done {
        std::fs::remove_file(&tmp).ok();
        return Err(format!("writing {}: {e}", dest.display()));
    }
    Ok(())
}

/// Builds a directory through `fill` beside `dest`, then puts it in place of
/// whatever `dest` was in one step, so a reader sees the old tree or the new
/// one and never a mix; the old tree is then removed.
pub fn replace_dir(dest: &Path, fill: impl FnOnce(&Path) -> io::Result<()>) -> Result<(), String> {
    let dir = dest
        .parent()
        .ok_or_else(|| format!("{} has no parent", dest.display()))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::remove_dir_all(&tmp).ok();
    let done = std::fs::create_dir(&tmp)
        .and_then(|()| fill(&tmp))
        .and_then(|()| put_in_place(&tmp, dest));
    // After a swap `tmp` holds the old tree; after a failure, the partial one.
    std::fs::remove_dir_all(&tmp).ok();
    done.map_err(|e| format!("writing {}: {e}", dest.display()))
}

/// Moves `from` to `to`, leaving whatever was at `to` at `from`.
fn put_in_place(from: &Path, to: &Path) -> io::Result<()> {
    if std::fs::symlink_metadata(to).is_err() {
        return std::fs::rename(from, to);
    }
    swap(from, to)
}

#[cfg(target_vendor = "apple")]
fn swap(a: &Path, b: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let a = CString::new(a.as_os_str().as_bytes()).map_err(io::Error::other)?;
    let b = CString::new(b.as_os_str().as_bytes()).map_err(io::Error::other)?;
    if unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_SWAP) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Without an atomic swap: the old tree steps aside first, so `b` is briefly
/// absent but never half-written.
#[cfg(not(target_vendor = "apple"))]
fn swap(a: &Path, b: &Path) -> io::Result<()> {
    let aside = a.with_extension("old");
    std::fs::rename(b, &aside)?;
    if let Err(e) = std::fs::rename(a, b) {
        std::fs::rename(&aside, b).ok();
        return Err(e);
    }
    std::fs::rename(&aside, a)
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
    fn replace_dir_swaps_whole_trees_and_leaves_nothing_behind() {
        let dir = scratch("replace-dir");
        let dest = dir.0.join("bin/Helper.app");
        let tree = |body: &'static str| {
            move |tmp: &Path| {
                std::fs::create_dir_all(tmp.join("Contents"))?;
                std::fs::write(tmp.join("Contents/file"), body)
            }
        };
        replace_dir(&dest, tree("a")).unwrap();
        std::fs::write(dest.join("Contents/stale"), "only in the old tree").unwrap();
        replace_dir(&dest, tree("b")).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.join("Contents/file")).unwrap(),
            "b"
        );
        assert!(!dest.join("Contents/stale").exists());

        let err = replace_dir(&dest, |tmp| {
            std::fs::write(tmp.join("half"), "x")?;
            Err(io::Error::other("no"))
        });
        assert!(err.is_err());
        assert_eq!(
            std::fs::read_to_string(dest.join("Contents/file")).unwrap(),
            "b"
        );
        assert_eq!(std::fs::read_dir(dir.0.join("bin")).unwrap().count(), 1);
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

    #[test]
    fn private_writes_narrow_an_existing_file() {
        let dir = scratch("write-private");
        let f = dir.0.join("session.cookie");
        write_private(&f, b"fresh").unwrap();
        assert_eq!(mode(&f), 0o600);
        assert!(is_owner_only(&f).unwrap());

        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!is_owner_only(&f).unwrap());
        write_private(&f, b"tok").unwrap();
        assert_eq!(mode(&f), 0o600);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "tok");
    }

    /// The vault's sealed file is written through `create_private`.
    #[cfg(feature = "server")]
    #[test]
    fn the_vault_file_is_owner_only() {
        let dir = scratch("vault-mode");
        let path = crate::paths::vault(&dir.0);
        crate::vault::Vault::new(&path, crate::vault::MasterKey::from_bytes([4; 32]))
            .store("voyage", "x")
            .unwrap();
        assert_eq!(mode(&path), 0o600);
    }
}

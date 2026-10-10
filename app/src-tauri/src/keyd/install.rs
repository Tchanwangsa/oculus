//! Installing, reinstalling and removing keyd's registration.

use std::path::{Path, PathBuf};

use keyd_core::paths;
use keyd_core::platform::{self, files};

use super::candidate::{installed_stamp, missing_bundled, sibling_of, source_hash_of};

#[derive(Debug, serde::Serialize)]
pub struct Installed {
    pub program: PathBuf,
    pub source_hash: String,
    /// Where the OS keeps the registration.
    pub plist: PathBuf,
}

/// Where the registration must point for `from`: `from` itself when it runs
/// in place, else its copy in `<data_dir>/bin`.
fn program_for(data_dir: &Path, from: &Path) -> PathBuf {
    if platform::registrar().runs_in_place(from) {
        from.to_path_buf()
    } else {
        paths::installed_bin(data_dir)
    }
}

/// Installs `from` and (re)loads its registration. A keyd that runs in place
/// is registered where it is; any other is copied to `<data_dir>/bin` first.
pub fn install(data_dir: &Path, from: &Path) -> Result<Installed, String> {
    let from = from
        .canonicalize()
        .map_err(|e| format!("{}: {e}", from.display()))?;
    let source_hash = source_hash_of(&from)?;
    install_hashed(data_dir, &from, source_hash)
}

/// `install`, unless the stamp already records `from`'s source and the
/// registration already runs where `from` would be installed. `None` when
/// nothing changed.
pub fn install_if_changed(data_dir: &Path, from: &Path) -> Result<Option<Installed>, String> {
    let from = from
        .canonicalize()
        .map_err(|e| format!("{}: {e}", from.display()))?;
    let source_hash = source_hash_of(&from)?;
    if is_current(data_dir, &from, &source_hash)? {
        return Ok(None);
    }
    install_hashed(data_dir, &from, source_hash).map(Some)
}

fn is_current(data_dir: &Path, from: &Path, source_hash: &str) -> Result<bool, String> {
    let program = platform::registrar().status()?.program;
    let wanted = program_for(data_dir, from);
    Ok(installed_stamp(data_dir).as_deref() == Some(source_hash)
        && program.as_deref() == Some(wanted.to_string_lossy().as_ref()))
}

fn install_hashed(data_dir: &Path, from: &Path, source_hash: String) -> Result<Installed, String> {
    let registrar = platform::registrar();
    registrar.check(data_dir)?;

    let program = program_for(data_dir, from);
    if program != from {
        // A new file renamed over the old one: overwriting a signed binary in
        // place leaves the kernel's cached signature stale and the next exec
        // is killed.
        files::replace_file(&program, |tmp| {
            std::fs::copy(from, tmp)?;
            files::set_executable(tmp)
        })?;
    }
    let plist = registrar.install(&program, data_dir)?;

    // Last, so a failed load is retried by the next preflight or launch.
    files::replace_file(&paths::stamp(data_dir), |tmp| {
        std::fs::write(tmp, format!("{source_hash}\n"))
    })?;
    Ok(Installed {
        program,
        source_hash,
        plist,
    })
}

/// Unloads keyd and removes its registration, dev binary, stamp and any
/// leftover endpoint. The vault and the keychain's master key stay, so a
/// reinstall reads the same secrets.
pub fn uninstall(data_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut removed = platform::registrar().uninstall()?;
    for p in [
        paths::installed_bin(data_dir),
        paths::stamp(data_dir),
        paths::socket(data_dir),
    ] {
        match std::fs::remove_file(&p) {
            Ok(()) => removed.push(p),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("removing {}: {e}", p.display())),
        }
    }
    Ok(removed)
}

/// Startup check. Dev builds do nothing: the preflight installs from the main
/// checkout only, and an app built in a worktree must not take keyd over.
/// A release reinstalls its bundled keyd when the stamp or the registered
/// program differs from it, and logs why when it cannot (`oculus keyd status`
/// reports a missing bundled keyd as well).
pub fn ensure_installed() {
    if cfg!(debug_assertions) {
        return;
    }
    std::thread::spawn(|| match ensure_bundled(&crate::paths::data_dir()) {
        Ok(Some(i)) => eprintln!(
            "[oculus] keyd installed from {} ({})",
            i.program.display(),
            &i.source_hash[..12]
        ),
        Ok(None) => {}
        Err(e) => eprintln!("[oculus] could not install keyd: {e}"),
    });
}

fn ensure_bundled(data_dir: &Path) -> Result<Option<Installed>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    ensure_bundled_beside(data_dir, &exe, cfg!(debug_assertions))
}

fn ensure_bundled_beside(
    data_dir: &Path,
    exe: &Path,
    debug_build: bool,
) -> Result<Option<Installed>, String> {
    if let Some(broken) = missing_bundled(exe, debug_build) {
        return Err(broken);
    }
    match sibling_of(exe).filter(|p| p.is_file()) {
        Some(bundled) => install_if_changed(data_dir, &bundled),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_keyd_is_refused_before_anything_is_written() {
        let dir = crate::test_support::Scratch::new("keyd-notkeyd");
        let err = source_hash_of(Path::new("/bin/echo")).unwrap_err();
        assert!(err.contains("not an oculus-keyd"), "{err}");
        assert!(install(&dir.join("data"), Path::new("/bin/echo")).is_err());
        assert!(install_if_changed(&dir.join("data"), Path::new("/bin/echo")).is_err());
        assert!(!dir.join("data").exists());
    }

    fn exe_in(name: &str) -> (crate::test_support::Scratch, PathBuf) {
        let dir = crate::test_support::Scratch::new(name);
        let macos = dir.join("Oculus.app/Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        let exe = macos.join("app");
        std::fs::write(&exe, "").unwrap();
        (dir, exe)
    }

    #[test]
    fn a_release_without_its_bundled_keyd_is_a_broken_install_not_a_silent_skip() {
        let (dir, exe) = exe_in("keyd-broken-install");
        let data = dir.join("data");

        let why = ensure_bundled_beside(&data, &exe, false).unwrap_err();
        assert!(why.contains("bundled oculus-keyd is missing"), "{why}");
        assert!(why.contains("Contents/MacOS/oculus-keyd"), "{why}");
        assert!(why.contains("reinstall"), "{why}");
        assert_eq!(missing_bundled(&exe, false), Some(why));
        assert!(!data.exists(), "nothing is written for a missing keyd");
    }

    #[test]
    fn a_debug_build_without_a_keyd_beside_it_is_not_an_error() {
        let (dir, exe) = exe_in("keyd-dev-no-sibling");
        assert!(ensure_bundled_beside(&dir.join("data"), &exe, true)
            .unwrap()
            .is_none());
        assert_eq!(missing_bundled(&exe, true), None);
    }

    #[test]
    fn a_bundled_file_that_is_not_keyd_is_an_error_too() {
        // A zero-byte sidecar placeholder that reached a bundle.
        let (dir, exe) = exe_in("keyd-placeholder");
        std::fs::write(sibling_of(&exe).unwrap(), "").unwrap();
        assert_eq!(missing_bundled(&exe, false), None);
        let why = ensure_bundled_beside(&dir.join("data"), &exe, false).unwrap_err();
        assert!(why.contains("Contents/MacOS/oculus-keyd"), "{why}");
        assert!(!dir.join("data").exists());
    }

    #[test]
    fn a_keyd_that_does_not_run_in_place_is_registered_from_the_data_dir() {
        let data = Path::new("/d");
        assert_eq!(
            program_for(data, Path::new("/x/target/signed/oculus-keyd")),
            Path::new("/d/bin/oculus-keyd")
        );
    }
}

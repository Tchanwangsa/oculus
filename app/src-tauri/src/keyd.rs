//! Installs and inspects `oculus-keyd`, the credential broker (its own crate
//! in `app/keyd/`; see docs/architecture.md and docs/development.md).
//!
//! An install registers keyd with the OS through `keyd_core`'s registrar,
//! pointing it at a fixed binary — `<data_dir>/bin/oculus-keyd` for a dev
//! build, copied there, or a bundle's own keyd, which must run in place for
//! its caller check — never at `target/` or a worktree. The source-hash stamp
//! in `<data_dir>/bin/` records what the registration runs, so a rebuild with
//! unchanged source never reinstalls. Nothing here is OS-specific: that is
//! `keyd_core::platform`.

use std::path::{Path, PathBuf};

use keyd_core::client::Client;
use keyd_core::paths::{self, BINARY};
use keyd_core::platform::{self, files};

/// What `<bin> source-hash` prints. Running it also proves the file is a keyd
/// that starts.
pub fn source_hash_of(bin: &Path) -> Result<String, String> {
    let out = std::process::Command::new(bin)
        .arg("source-hash")
        .output()
        .map_err(|e| format!("running {}: {e}", bin.display()))?;
    let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{} is not an oculus-keyd (source-hash gave {:?})", bin.display(), hash));
    }
    Ok(hash)
}

pub fn installed_stamp(data_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(paths::stamp(data_dir)).ok()?;
    Some(text.trim().to_string()).filter(|s| !s.is_empty())
}

/// The keyd this build of the app or CLI would install: the one beside it in
/// the bundle, else (debug builds) the signed output of `bun run keyd` in the
/// checkout it was built from.
pub fn candidate() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let sibling = exe.parent()?.join(BINARY);
    if sibling.is_file() {
        return Some(sibling);
    }
    if cfg!(debug_assertions) {
        let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../keyd/target/signed").join(BINARY);
        return built.canonicalize().ok().filter(|p| p.is_file());
    }
    None
}

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
    let from = from.canonicalize().map_err(|e| format!("{}: {e}", from.display()))?;
    let source_hash = source_hash_of(&from)?;
    install_hashed(data_dir, &from, source_hash)
}

/// `install`, unless the stamp already records `from`'s source and the
/// registration already runs where `from` would be installed. `None` when
/// nothing changed.
pub fn install_if_changed(data_dir: &Path, from: &Path) -> Result<Option<Installed>, String> {
    let from = from.canonicalize().map_err(|e| format!("{}: {e}", from.display()))?;
    let source_hash = source_hash_of(&from)?;
    if is_current(data_dir, &from, &source_hash)? {
        return Ok(None);
    }
    install_hashed(data_dir, &from, source_hash).map(Some)
}

fn is_current(data_dir: &Path, from: &Path, source_hash: &str) -> Result<bool, String> {
    let program = platform::registrar().status()?.program;
    let wanted = program_for(data_dir, from);
    Ok(installed_stamp(data_dir).as_deref() == Some(source_hash) && program.as_deref() == Some(wanted.to_string_lossy().as_ref()))
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
    files::replace_file(&paths::stamp(data_dir), |tmp| std::fs::write(tmp, format!("{source_hash}\n")))?;
    Ok(Installed { program, source_hash, plist })
}

/// Unloads keyd and removes its registration, dev binary, stamp and any
/// leftover endpoint. The vault and the keychain's master key stay, so a
/// reinstall reads the same secrets.
pub fn uninstall(data_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut removed = platform::registrar().uninstall()?;
    for p in [paths::installed_bin(data_dir), paths::stamp(data_dir), paths::socket(data_dir)] {
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
/// program differs from it.
pub fn ensure_installed() {
    if cfg!(debug_assertions) {
        return;
    }
    std::thread::spawn(|| match ensure_bundled(&crate::paths::data_dir()) {
        Ok(Some(i)) => eprintln!("[oculus] keyd installed from {} ({})", i.program.display(), &i.source_hash[..12]),
        Ok(None) => {}
        Err(e) => eprintln!("[oculus] could not install keyd: {e}"),
    });
}

fn ensure_bundled(data_dir: &Path) -> Result<Option<Installed>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundled = exe.with_file_name(BINARY);
    if !bundled.is_file() {
        return Ok(None);
    }
    install_if_changed(data_dir, &bundled)
}

// ── Status ───────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct Status {
    pub plist: PathBuf,
    /// The binary the registration runs, when one is installed.
    pub program: Option<String>,
    pub loaded: bool,
    pub installed_hash: Option<String>,
    /// What this build would install, and its source hash or why it has none.
    pub candidate: Option<PathBuf>,
    pub candidate_hash: Option<String>,
    pub candidate_error: Option<String>,
    pub socket: PathBuf,
    /// keyd's `ping` reply: version, source hash, pid.
    pub ping: Option<serde_json::Value>,
    pub ping_error: Option<String>,
    pub vault: PathBuf,
    pub vault_bytes: Option<u64>,
}

/// Never reads a secret: `ping` is the one op it sends, and keyd answers it
/// without opening the vault or the keychain.
pub fn status(data_dir: &Path) -> Result<Status, String> {
    let registration = platform::registrar().status()?;
    let candidate = candidate();
    let (candidate_hash, candidate_error) = match &candidate {
        Some(c) => match source_hash_of(c) {
            Ok(h) => (Some(h), None),
            Err(e) => (None, Some(e)),
        },
        None => (None, None),
    };
    let (ping, ping_error) = match Client::at(data_dir).ping() {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e)),
    };
    let vault = paths::vault(data_dir);
    Ok(Status {
        loaded: registration.loaded,
        plist: registration.path,
        program: registration.program,
        installed_hash: installed_stamp(data_dir),
        candidate,
        candidate_hash,
        candidate_error,
        vault_bytes: std::fs::metadata(&vault).ok().map(|m| m.len()),
        vault,
        socket: paths::socket(data_dir),
        ping,
        ping_error,
    })
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

    #[test]
    fn a_keyd_that_does_not_run_in_place_is_registered_from_the_data_dir() {
        let data = Path::new("/d");
        assert_eq!(program_for(data, Path::new("/x/target/signed/oculus-keyd")), Path::new("/d/bin/oculus-keyd"));
    }
}

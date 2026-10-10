//! The keyd a build would install, and the stamp of the one installed.

use std::path::{Path, PathBuf};

use keyd_core::paths::{self, BINARY};

/// What `<bin> source-hash` prints. Running it also proves the file is a keyd
/// that starts.
pub fn source_hash_of(bin: &Path) -> Result<String, String> {
    let out = std::process::Command::new(bin)
        .arg("source-hash")
        .output()
        .map_err(|e| format!("running {}: {e}", bin.display()))?;
    let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "{} is not an oculus-keyd (source-hash gave {:?})",
            bin.display(),
            hash
        ));
    }
    Ok(hash)
}

pub fn installed_stamp(data_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(paths::stamp(data_dir)).ok()?;
    Some(text.trim().to_string()).filter(|s| !s.is_empty())
}

/// The keyd beside an executable: where a bundle puts it.
pub(super) fn sibling_of(exe: &Path) -> Option<PathBuf> {
    exe.parent().map(|dir| dir.join(BINARY))
}

/// The keyd this build of the app or CLI would install: the one beside it in
/// the bundle, else (debug builds) the signed output of `bun run keyd` in the
/// checkout it was built from.
pub fn candidate() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let sibling = sibling_of(&exe)?;
    if sibling.is_file() {
        return Some(sibling);
    }
    if cfg!(debug_assertions) {
        let built = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../keyd/target/signed")
            .join(BINARY);
        return built.canonicalize().ok().filter(|p| p.is_file());
    }
    None
}

/// Why `exe` has no keyd to install, when a release build should: its bundle
/// ships keyd beside it, so a missing file is a broken install, and without it
/// every credential call quietly falls back to the keychain. A debug build
/// has none until `bun run keyd` runs, which is no fault.
pub(super) fn missing_bundled(exe: &Path, debug_build: bool) -> Option<String> {
    let sibling = sibling_of(exe)?;
    if debug_build || sibling.is_file() {
        return None;
    }
    Some(format!(
        "the bundled {BINARY} is missing ({}) — this Oculus install is broken, \
         and credentials fall back to the keychain; reinstall Oculus",
        sibling.display()
    ))
}

/// Why this build has no keyd to install, when that is a broken install.
pub fn no_candidate_reason() -> Option<String> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    missing_bundled(&exe, cfg!(debug_assertions))
}

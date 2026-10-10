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

/// The keyd this build of the app or CLI would install: its bundle's, else
/// (debug builds) the helper app `bun run keyd` signed in the checkout it was
/// built from.
pub fn candidate() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    if let Some(bundled) = paths::bundled_program(&exe).filter(|p| p.is_file()) {
        return Some(bundled);
    }
    if cfg!(debug_assertions) {
        let built = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../keyd/target/signed")
            .join(paths::helper_app_name());
        return paths::helper_program(&built)
            .canonicalize()
            .ok()
            .filter(|p| p.is_file());
    }
    None
}

/// Why `exe` has no keyd to install, when a release build should: its bundle
/// ships keyd in a helper app, so a missing file is a broken install, and
/// without it every credential call quietly falls back to the keychain. A
/// debug build has none until `bun run keyd` runs, which is no fault.
pub(super) fn missing_bundled(exe: &Path, debug_build: bool) -> Option<String> {
    let bundled = paths::bundled_program(exe)?;
    if debug_build || bundled.is_file() {
        return None;
    }
    Some(format!(
        "the bundled {BINARY} is missing ({}) — this Oculus install is broken, \
         and credentials fall back to the keychain; reinstall Oculus",
        bundled.display()
    ))
}

/// Why this build has no keyd to install, when that is a broken install.
pub fn no_candidate_reason() -> Option<String> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    missing_bundled(&exe, cfg!(debug_assertions))
}

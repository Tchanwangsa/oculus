//! Binding to libpdfium: found once per process, then shared.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use pdfium_render::prelude::Pdfium;

use super::RasterError;

/// `Pdfium::new` may only be called once per process, so the binding is a
/// singleton (`Send + Sync` under the `thread_safe` feature).
static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();

pub(super) fn pdfium() -> Result<&'static Pdfium, RasterError> {
    match PDFIUM.get_or_init(bind) {
        Ok(pdfium) => Ok(pdfium),
        Err(message) => Err(RasterError::Library(message.clone())),
    }
}

fn bind() -> Result<Pdfium, String> {
    let mut tried = Vec::new();
    for candidate in library_candidates() {
        if !candidate.exists() {
            continue;
        }
        match Pdfium::bind_to_library(&candidate) {
            Ok(bindings) => return Ok(Pdfium::new(bindings)),
            Err(error) => tried.push(format!("{} ({error})", candidate.display())),
        }
    }

    // Last resort: a system pdfium. Its version may not match the pinned one,
    // and a mismatch fails at bind time.
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(error) => {
            tried.push(format!("system library ({error})"));
            Err(format!(
                "no usable libpdfium; run `bun run pdfium` in app/ to fetch one. Tried: {}",
                tried.join("; ")
            ))
        }
    }
}

/// Where to look for the library, in order: `OCULUS_PDFIUM_LIB` (the dylib
/// or its directory); the bundle's `Contents/Frameworks/`; beside the
/// executable; `binaries/` in an ancestor (dev builds under `target/`); and the
/// compile-time manifest dir. Relative to `current_exe()`, never the cwd.
fn library_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let file_name = Pdfium::pdfium_platform_library_name();

    if let Some(explicit) = std::env::var_os("OCULUS_PDFIUM_LIB") {
        let path = PathBuf::from(explicit);
        if path.is_dir() {
            candidates.push(path.join(&file_name));
        } else {
            candidates.push(path);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Bundled .app: Contents/MacOS/Oculus -> Contents/Frameworks/.
            candidates.push(dir.join("../Frameworks").join(&file_name));
            // Beside the executable, for a flat install layout.
            candidates.push(dir.join(&file_name));
            // Dev: target/debug/app, target/debug/deps/<test> -> src-tauri/binaries.
            push_ancestor_binaries(&mut candidates, dir, &file_name);
        }
    }

    // What `cargo test` normally hits; absent in release builds.
    candidates.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(&file_name),
    );

    candidates
}

fn push_ancestor_binaries(candidates: &mut Vec<PathBuf>, from: &Path, file_name: &OsString) {
    for ancestor in from.ancestors().take(6) {
        candidates.push(ancestor.join("binaries").join(file_name));
    }
}

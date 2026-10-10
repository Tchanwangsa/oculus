//! Where libpdfium is loaded from, in order.
//!
//! A release bundle loads it only from `Contents/Frameworks/` (or beside the
//! executable). The app and CLI run with library validation off, so any
//! dylib they `dlopen` is accepted, and an environment variable, a nearby
//! folder or the system's search path must not be able to name one
//! (docs/development.md). Those sources are compiled only where a developer
//! runs the build: debug, test, or release with the `dev-pdfium` feature
//! (`bun run cli`). Paths are relative to `current_exe()`, never the cwd.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use pdfium_render::prelude::Pdfium;

pub(super) fn candidates() -> Vec<PathBuf> {
    #[cfg(any(debug_assertions, test, feature = "dev-pdfium"))]
    let explicit = std::env::var_os("OCULUS_PDFIUM_LIB");
    #[cfg(not(any(debug_assertions, test, feature = "dev-pdfium")))]
    let explicit = None;
    let exe = std::env::current_exe().ok();
    ordered(
        &Pdfium::pdfium_platform_library_name(),
        explicit,
        exe.as_deref(),
    )
}

/// The bundle's `Contents/Frameworks/` and the flat layout beside the
/// executable, then (dev) `OCULUS_PDFIUM_LIB` first, an ancestor `binaries/`
/// and the build tree's after.
#[cfg_attr(
    not(any(debug_assertions, test, feature = "dev-pdfium")),
    allow(unused_variables)
)]
fn ordered(file_name: &OsString, explicit: Option<OsString>, exe: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    #[cfg(any(debug_assertions, test, feature = "dev-pdfium"))]
    if let Some(explicit) = explicit {
        // The dylib itself, or the directory holding it.
        let path = PathBuf::from(explicit);
        if path.is_dir() {
            candidates.push(path.join(file_name));
        } else {
            candidates.push(path);
        }
    }

    if let Some(dir) = exe.and_then(Path::parent) {
        // Bundled .app: Contents/MacOS/Oculus -> Contents/Frameworks/.
        candidates.push(dir.join("../Frameworks").join(file_name));
        candidates.push(dir.join(file_name));

        // Dev: target/debug/app, target/debug/deps/<test> -> src-tauri/binaries.
        #[cfg(any(debug_assertions, test, feature = "dev-pdfium"))]
        for ancestor in dir.ancestors().take(6) {
            candidates.push(ancestor.join("binaries").join(file_name));
        }
    }

    // What a build tree finds wherever its binary was moved to (a CLI linked
    // into ~/.local/bin, or `cargo test`).
    #[cfg(any(debug_assertions, test, feature = "dev-pdfium"))]
    candidates.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(file_name),
    );

    candidates
}

/// Last resort, dev builds only: a system pdfium, whose version may not match
/// the pinned one (a mismatch fails at bind time). `tried` is what failed so
/// far, for the error.
pub(super) fn bind_system(
    #[cfg_attr(
        not(any(debug_assertions, test, feature = "dev-pdfium")),
        allow(unused_mut)
    )]
    mut tried: Vec<String>,
) -> Result<Pdfium, String> {
    #[cfg(any(debug_assertions, test, feature = "dev-pdfium"))]
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => return Ok(Pdfium::new(bindings)),
        Err(error) => tried.push(format!("system library ({error})")),
    }
    Err(format!(
        "no usable libpdfium; run `bun run pdfium` in app/ to fetch one. Tried: {}",
        tried.join("; ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name() -> OsString {
        OsString::from("libpdfium.dylib")
    }

    #[test]
    fn a_bundle_looks_in_frameworks_then_beside_itself() {
        let list = ordered(
            &name(),
            None,
            Some(Path::new("/Applications/Oculus.app/Contents/MacOS/oculus")),
        );
        assert_eq!(
            list[..2],
            [
                PathBuf::from(
                    "/Applications/Oculus.app/Contents/MacOS/../Frameworks/libpdfium.dylib"
                ),
                PathBuf::from("/Applications/Oculus.app/Contents/MacOS/libpdfium.dylib"),
            ]
        );
    }

    /// The dev order, which only a build with the gate open compiles.
    #[test]
    fn the_dev_sources_follow_the_override_and_the_bundle_in_a_fixed_order() {
        let exe = Path::new("/w/app/src-tauri/target/debug/oculus");
        let built = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries/libpdfium.dylib");

        // `/nonexistent` is not a directory, so it is taken as the dylib.
        let list = ordered(&name(), Some("/nonexistent/lib.dylib".into()), Some(exe));
        let mut want = vec![
            PathBuf::from("/nonexistent/lib.dylib"),
            PathBuf::from("/w/app/src-tauri/target/debug/../Frameworks/libpdfium.dylib"),
            PathBuf::from("/w/app/src-tauri/target/debug/libpdfium.dylib"),
        ];
        for ancestor in [
            "/w/app/src-tauri/target/debug",
            "/w/app/src-tauri/target",
            "/w/app/src-tauri",
            "/w/app",
            "/w",
            "/",
        ] {
            want.push(PathBuf::from(ancestor).join("binaries/libpdfium.dylib"));
        }
        want.push(built);
        assert_eq!(list, want);

        // A directory override names the library inside it.
        let dir = std::env::temp_dir();
        assert_eq!(
            ordered(&name(), Some(dir.clone().into()), None)[0],
            dir.join("libpdfium.dylib")
        );
        // With no override or executable, only the build tree remains.
        assert_eq!(ordered(&name(), None, None).len(), 1);
    }
}

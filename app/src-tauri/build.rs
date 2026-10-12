use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn main() {
    studio_render_build();
    let attributes = tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new()
            .app_manifest(include_str!("windows-app-manifest.xml")),
    );
    tauri_build::try_build(attributes).expect("building Oculus resources failed");
}

/// `STUDIO_RENDER_BUILD`: sha256 hex of everything a segment's pixels can
/// depend on beyond the project: studio-core's `src/`, its bundled `fonts/`
/// and `themes/`, the images a `starter:` src names (`library/assets/`,
/// compiled in), the KaTeX port `<Math>` typesets with (a path dependency,
/// so no version in the lock moves when it changes), `src/studio/render/`,
/// and `Cargo.lock`, which pins the versions of vello_cpu, parley, taffy and
/// the rest. Each root's path, then
/// its files' sorted relative paths and bytes, all length-prefixed. A
/// segment's key carries it, so a change to any of them never reuses a
/// segment the old build made, and the app and the CLI built from one tree
/// share segments.
fn studio_render_build() {
    let roots = [
        Path::new("../studio-core/src"),
        Path::new("../studio-core/fonts"),
        Path::new("../studio-core/themes"),
        Path::new("../studio-core/library/assets"),
        Path::new("../math-core/katex/src"),
        Path::new("../math-core/katex/data"),
        Path::new("../math-core/katex/build.rs"),
        Path::new("src/studio/render"),
        Path::new("Cargo.lock"),
    ];
    let mut hash = Sha256::new();
    let mut part = |bytes: &[u8]| {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    };
    for root in roots {
        println!("cargo:rerun-if-changed={}", root.display());
        part(root.to_string_lossy().as_bytes());
        let mut files = Vec::new();
        if root.is_file() {
            files.push(root.to_path_buf());
        } else {
            walk(root, &mut files);
        }
        files.sort();
        for file in files {
            let bytes = std::fs::read(&file).expect("read a studio render input");
            let rel = file.strip_prefix(root).unwrap_or(&file).to_string_lossy();
            part(rel.as_bytes());
            part(&bytes);
        }
    }
    println!("cargo:rustc-env=STUDIO_RENDER_BUILD={:x}", hash.finalize());
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if entry.file_name() != ".DS_Store" {
            out.push(path);
        }
    }
}

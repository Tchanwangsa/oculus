use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn main() {
    studio_render_build();
    tauri_build::build()
}

/// `STUDIO_RENDER_BUILD`: sha256 hex of every file under studio-core's
/// `src/` and `src/studio/render/` (sorted relative paths, each followed by
/// its bytes). A render segment's key carries it, so a renderer or encoder
/// change never reuses a segment the old code made, and the app and the CLI
/// built from one tree share segments.
fn studio_render_build() {
    let roots = [
        Path::new("../studio-core/src"),
        Path::new("src/studio/render"),
    ];
    let mut hash = Sha256::new();
    for root in roots {
        println!("cargo:rerun-if-changed={}", root.display());
        let mut files = Vec::new();
        walk(root, &mut files);
        files.sort();
        for file in files {
            let bytes = std::fs::read(&file).expect("read a studio source file");
            let rel = file.strip_prefix(root).unwrap_or(&file).to_string_lossy();
            for part in [rel.as_bytes(), &bytes] {
                hash.update((part.len() as u64).to_le_bytes());
                hash.update(part);
            }
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

//! Embeds `KEYD_SOURCE_HASH`: a sha256 over every input that decides keyd's
//! bytes — keyd's and core's sources and manifests, keyd's Cargo.lock and
//! its cargo config. Paths are hashed relative to this crate, so every
//! checkout of the same commit agrees. The installer compares this, not the
//! binary, because a toolchain change alters bytes without a source change
//! (docs/development.md).

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    let mut files = Vec::new();
    for top in ["Cargo.toml", "Cargo.lock", "build.rs", ".cargo/config.toml", "core/Cargo.toml"] {
        files.push(root.join(top));
    }
    for dir in ["src", "core/src"] {
        println!("cargo:rerun-if-changed={}", root.join(dir).display());
        walk(&root.join(dir), &mut files);
    }
    let mut rel: Vec<(String, PathBuf)> = files
        .into_iter()
        .map(|p| (p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"), p))
        .collect();
    rel.sort();

    let mut hash = Sha256::new();
    for (name, path) in &rel {
        println!("cargo:rerun-if-changed={}", path.display());
        let body = std::fs::read(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        hash.update(name.as_bytes());
        hash.update([0]);
        hash.update((body.len() as u64).to_le_bytes());
        hash.update(&body);
    }
    let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    println!("cargo:rustc-env=KEYD_SOURCE_HASH={hex}");
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

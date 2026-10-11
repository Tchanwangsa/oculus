//! A segment's content address. A key that misses an input renders a stale
//! video silently, so it takes everything a scene's frames can depend on:
//! studio-core's fingerprint of the scene (its spec, words and frame count,
//! the scene before it, the resolved theme), every file under
//! `components/` and `assets/` (an image may change under its name), the
//! render settings, and the build: `STUDIO_RENDER_BUILD`, a hash of
//! studio-core's and this module's sources (build.rs).

use std::path::Path;

use sha2::{Digest, Sha256};

use super::error::RenderError;
use super::quality::Settings;

/// Bumped when how a segment is made changes in a way its inputs do not
/// show, so every segment renders again.
const SCHEME: &str = "oculus-render-segment-1";

/// The renderer's and encoder's sources, hashed by build.rs.
const BUILD: &str = env!("STUDIO_RENDER_BUILD");

/// The trees every segment depends on, hashed once per render.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Trees {
    pub components: String,
    pub assets: String,
}

impl Trees {
    pub fn of(dir: &Path) -> Result<Self, RenderError> {
        Ok(Self {
            components: tree_hash(&dir.join("components"))?,
            assets: tree_hash(&dir.join("assets"))?,
        })
    }
}

/// sha256 hex over each length-prefixed part.
fn hash_parts<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> String {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    format!("{:x}", hash.finalize())
}

/// sha256 hex of every file under `dir`: relative paths (with `/`) in
/// sorted order, each followed by its bytes. A missing folder hashes as
/// empty. Finder's `.DS_Store` is skipped.
pub(super) fn tree_hash(dir: &Path) -> Result<String, RenderError> {
    let mut files = Vec::new();
    walk(dir, "", &mut files)?;
    files.sort();
    let mut hash = Sha256::new();
    for rel in &files {
        let path = dir.join(rel);
        let bytes = std::fs::read(&path).map_err(|e| RenderError::io(&path, e))?;
        for part in [rel.as_bytes(), &bytes] {
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part);
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<(), RenderError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(RenderError::io(dir, e)),
    };
    for entry in entries {
        let entry = entry.map_err(|e| RenderError::io(dir, e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ".DS_Store" {
            continue;
        }
        let rel = format!("{prefix}{name}");
        let path = entry.path();
        if path.is_dir() {
            walk(&path, &format!("{rel}/"), out)?;
        } else {
            out.push(rel);
        }
    }
    Ok(())
}

/// Scene `fingerprint`'s segment key under `trees` and `settings`.
pub(super) fn segment_key(fingerprint: &[u8], trees: &Trees, settings: &Settings) -> String {
    let settings = settings.key_text();
    hash_parts([
        SCHEME.as_bytes(),
        BUILD.as_bytes(),
        fingerprint,
        trees.components.as_bytes(),
        trees.assets.as_bytes(),
        settings.as_bytes(),
    ])
}

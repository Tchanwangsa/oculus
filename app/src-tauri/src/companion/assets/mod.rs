//! The browser bundle under `/c/`: `dist/companion/**`, built by
//! `vite.companion.config.ts`. A release serves the copy Tauri embeds with
//! the frontend; a debug build reads `app/dist/companion` from disk, so a
//! rebuilt bundle is served without rebuilding Rust. Lookups go through the
//! allow-list in `path.rs` and a known-key set (embedded) or a canonical
//! root check (disk), never through Tauri's fallback to the app's own
//! `index.html`.

mod mime;
mod path;

use bytes::Bytes;
use std::collections::HashSet;
use std::path::PathBuf;

/// The bundle's prefix inside `dist/`.
const PREFIX: &str = "companion/";

pub enum Lookup {
    Found {
        bytes: Bytes,
        mime: &'static str,
    },
    /// Not on the allow-list, or no such file.
    Missing,
    /// The bundle itself is not there: never built, or not embedded.
    NoBundle,
}

type Embedded = Box<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;

enum Source {
    Disk(PathBuf),
    Embedded {
        keys: HashSet<String>,
        get: Embedded,
    },
}

pub struct Assets {
    source: Source,
}

impl Assets {
    /// A bundle directory on disk (`index.html`, `assets/`).
    pub fn disk(root: PathBuf) -> Self {
        Self {
            source: Source::Disk(root),
        }
    }

    /// Embedded assets: every key Tauri embedded, and a reader for one.
    pub fn embedded(
        keys: impl Iterator<Item = String>,
        get: impl Fn(&str) -> Option<Vec<u8>> + Send + Sync + 'static,
    ) -> Self {
        let keys = keys
            .filter_map(|key| {
                key.trim_start_matches('/')
                    .strip_prefix(PREFIX)
                    .map(str::to_string)
            })
            .collect();
        Self {
            source: Source::Embedded {
                keys,
                get: Box::new(get),
            },
        }
    }

    /// The app's bundle: on disk beside the crate in debug builds, embedded
    /// in release ones.
    pub fn for_app<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Self {
        if cfg!(debug_assertions) {
            return Self::disk(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist/companion"));
        }
        let resolver = app.asset_resolver();
        let keys: Vec<String> = resolver.iter().map(|(key, _)| key.to_string()).collect();
        Self::embedded(keys.into_iter(), move |rel| {
            resolver
                .get(format!("{PREFIX}{rel}"))
                .map(|asset| asset.bytes)
        })
    }

    /// The asset for the part of the URL after `/c/`. Blocking: it may read a
    /// file or decompress an embedded one.
    pub fn get(&self, rest: &str) -> Lookup {
        let Some(rel) = path::normalise(rest) else {
            return Lookup::Missing;
        };
        let Some(mime) = mime::for_path(&rel) else {
            return Lookup::Missing;
        };
        let bytes = match &self.source {
            Source::Disk(root) => match read_under(root, &rel) {
                Ok(bytes) => bytes,
                Err(lookup) => return lookup,
            },
            Source::Embedded { keys, get } => {
                if keys.is_empty() {
                    return Lookup::NoBundle;
                }
                if !keys.contains(&rel) {
                    return Lookup::Missing;
                }
                match get(&rel) {
                    Some(bytes) => bytes,
                    None => return Lookup::Missing,
                }
            }
        };
        Lookup::Found {
            bytes: Bytes::from(bytes),
            mime,
        }
    }
}

/// Read `rel` only if it resolves, symlinks followed, to a file inside `root`.
fn read_under(root: &std::path::Path, rel: &str) -> Result<Vec<u8>, Lookup> {
    let Ok(root) = root.canonicalize() else {
        return Err(Lookup::NoBundle);
    };
    let Ok(file) = root.join(rel).canonicalize() else {
        return Err(Lookup::Missing);
    };
    if !file.starts_with(&root) || !file.is_file() {
        return Err(Lookup::Missing);
    }
    std::fs::read(&file).map_err(|_| Lookup::Missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    fn found(lookup: Lookup) -> Option<(Vec<u8>, &'static str)> {
        match lookup {
            Lookup::Found { bytes, mime } => Some((bytes.to_vec(), mime)),
            _ => None,
        }
    }

    #[test]
    fn disk_bundle_serves_only_files_inside_it() {
        let scratch = Scratch::new("companion-assets");
        let root = scratch.join("dist/companion");
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("index.html"), "<html>").unwrap();
        std::fs::write(root.join("assets/a.wasm"), b"\0asm").unwrap();
        std::fs::write(scratch.join("outside.js"), "secret").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(scratch.join("outside.js"), root.join("assets/link.js"))
            .unwrap();
        let assets = Assets::disk(root);
        assert_eq!(
            found(assets.get("")),
            Some((b"<html>".to_vec(), "text/html; charset=utf-8"))
        );
        assert_eq!(
            found(assets.get("assets/a.wasm")).unwrap().1,
            "application/wasm"
        );
        assert!(matches!(assets.get("assets/link.js"), Lookup::Missing));
        assert!(matches!(
            assets.get("assets/../../outside.js"),
            Lookup::Missing
        ));
        assert!(matches!(assets.get("assets/none.js"), Lookup::Missing));
        assert!(matches!(
            Assets::disk(scratch.join("absent")).get(""),
            Lookup::NoBundle
        ));
    }

    #[test]
    fn embedded_bundle_serves_only_its_own_keys() {
        let assets = Assets::embedded(
            [
                "/index.html",
                "/companion/index.html",
                "/companion/assets/a.js",
            ]
            .into_iter()
            .map(String::from),
            |rel| Some(rel.as_bytes().to_vec()),
        );
        assert_eq!(found(assets.get("")).unwrap().0, b"index.html");
        assert_eq!(found(assets.get("assets/a.js")).unwrap().0, b"assets/a.js");
        assert!(matches!(assets.get("assets/b.js"), Lookup::Missing));
        let empty = Assets::embedded(std::iter::once("/index.html".into()), |_| None);
        assert!(matches!(empty.get(""), Lookup::NoBundle));
    }
}

//! The widget kit (`dist/widget-kit/**`, built by the frontend) served to
//! sandboxed widget frames. A release reads the copy Tauri embeds with the
//! frontend; a debug build reads `app/dist/widget-kit` from disk, so a
//! rebuilt kit is served without rebuilding Rust (same split as the
//! companion bundle, `companion/assets`).
//!
//! The decision is a pure function of method and path (`respond`), so the
//! guard and the headers are tested without a socket.

use crate::companion::assets::mime;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// URL prefix on the media server, and the key prefix inside `dist/`.
pub const ROUTE: &str = "/widget-kit/";
const PREFIX: &str = "widget-kit/";

/// Headers on every kit response, and on no other route: a sandboxed frame's
/// origin is `null`, so its module scripts need CORS. The token-gated video
/// route never gets them. Files are unhashed, hence `no-cache`.
pub const HEADERS: &[(&str, &str)] = &[
    ("Access-Control-Allow-Origin", "*"),
    ("X-Content-Type-Options", "nosniff"),
    ("Cache-Control", "no-cache"),
];

type Embedded = Box<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;

enum Source {
    Disk(PathBuf),
    Embedded {
        keys: HashSet<String>,
        get: Embedded,
    },
}

pub struct Kit {
    source: Source,
}

pub struct Reply {
    pub status: u16,
    pub content_type: Option<&'static str>,
    pub body: Vec<u8>,
}

impl Kit {
    pub fn disk(root: PathBuf) -> Self {
        Self {
            source: Source::Disk(root),
        }
    }

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

    /// The app's kit: on disk beside the crate in debug builds, embedded in
    /// release ones.
    pub fn for_app<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Self {
        if cfg!(debug_assertions) {
            return Self::disk(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist/widget-kit"),
            );
        }
        let resolver = app.asset_resolver();
        let keys: Vec<String> = resolver.iter().map(|(key, _)| key.to_string()).collect();
        Self::embedded(keys.into_iter(), move |rel| {
            resolver
                .get(format!("{PREFIX}{rel}"))
                .map(|asset| asset.bytes)
        })
    }

    /// The reply to `method` for the part of the URL path after `ROUTE`.
    /// Blocking: reads a file or decompresses an embedded one.
    pub fn respond(&self, method: &str, rest: &str) -> Reply {
        let missing = Reply {
            status: 404,
            content_type: None,
            body: Vec::new(),
        };
        if method != "GET" && method != "HEAD" {
            return Reply {
                status: 405,
                ..missing
            };
        }
        let Some(rel) = normalise(rest) else {
            return missing;
        };
        let Some(content_type) = mime::for_path(&rel) else {
            return missing;
        };
        let bytes = match &self.source {
            Source::Disk(root) => read_under(root, &rel),
            Source::Embedded { keys, get } => keys.contains(&rel).then(|| get(&rel)).flatten(),
        };
        match bytes {
            Some(body) => Reply {
                status: 200,
                content_type: Some(content_type),
                body,
            },
            None => missing,
        }
    }
}

/// The kit-relative path for the URL path after `ROUTE`, or `None`. Every
/// segment is plain characters, so `..`, `.`, hidden files, empty segments,
/// backslashes, NUL, percent-escapes and drive letters never reach a lookup.
fn normalise(rest: &str) -> Option<String> {
    let plain = !rest.is_empty()
        && rest.split('/').all(|segment| {
            !segment.is_empty()
                && !segment.starts_with('.')
                && segment.bytes().all(|b| {
                    b.is_ascii_alphanumeric()
                        || matches!(b, b'.' | b'-' | b'_' | b'~' | b'@' | b'+')
                })
        });
    plain.then(|| rest.to_owned())
}

/// Read `rel` only if it resolves, symlinks followed, to a file inside `root`.
fn read_under(root: &Path, rel: &str) -> Option<Vec<u8>> {
    let root = root.canonicalize().ok()?;
    let file = root.join(rel).canonicalize().ok()?;
    if !file.starts_with(&root) || !file.is_file() {
        return None;
    }
    std::fs::read(&file).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    fn kit_with_files() -> (Scratch, Kit) {
        let scratch = Scratch::new("widget-kit");
        let root = scratch.join("dist/widget-kit");
        std::fs::create_dir_all(root.join("chunks")).unwrap();
        std::fs::write(root.join("core.js"), "export {}").unwrap();
        std::fs::write(root.join("chunks/math.wasm"), b"\0asm").unwrap();
        std::fs::write(root.join("kit.css"), "a{}").unwrap();
        std::fs::write(root.join("notes.txt"), "x").unwrap();
        std::fs::write(scratch.join("outside.js"), "secret").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(scratch.join("outside.js"), root.join("link.js")).unwrap();
        let kit = Kit::disk(root);
        (scratch, kit)
    }

    #[test]
    fn serves_kit_files_with_their_types() {
        let (_scratch, kit) = kit_with_files();
        let js = kit.respond("GET", "core.js");
        assert_eq!(js.status, 200);
        assert_eq!(js.content_type, Some("text/javascript; charset=utf-8"));
        assert_eq!(js.body, b"export {}");
        assert_eq!(
            kit.respond("HEAD", "chunks/math.wasm").content_type,
            Some("application/wasm")
        );
        assert_eq!(
            kit.respond("GET", "kit.css").content_type,
            Some("text/css; charset=utf-8")
        );
    }

    #[test]
    fn traversal_and_odd_paths_are_refused() {
        let (_scratch, kit) = kit_with_files();
        for bad in [
            "",
            "../outside.js",
            "chunks/../../outside.js",
            "chunks/..",
            "./core.js",
            "chunks//math.wasm",
            "/core.js",
            "/etc/passwd",
            "a\\b.js",
            "%2e%2e/outside.js",
            "core%2ejs",
            ".hidden.js",
            "a\0.js",
            "C:x.js",
            "a b.js",
            "link.js",
            "none.js",
            "notes.txt",
            "chunks",
        ] {
            assert_eq!(kit.respond("GET", bad).status, 404, "{bad:?}");
        }
    }

    #[test]
    fn only_get_and_head() {
        let (_scratch, kit) = kit_with_files();
        assert_eq!(kit.respond("POST", "core.js").status, 405);
        assert_eq!(kit.respond("HEAD", "core.js").status, 200);
    }

    #[test]
    fn embedded_kit_serves_only_its_keys() {
        let kit = Kit::embedded(
            [
                "/widget-kit/core.js".to_owned(),
                "/companion/a.js".to_owned(),
            ]
            .into_iter(),
            |rel| (rel == "core.js").then(|| b"x".to_vec()),
        );
        assert_eq!(kit.respond("GET", "core.js").status, 200);
        assert_eq!(kit.respond("GET", "a.js").status, 404);
    }

    #[test]
    fn headers_allow_a_null_origin_module_load() {
        assert!(HEADERS.contains(&("Access-Control-Allow-Origin", "*")));
        assert!(HEADERS.contains(&("X-Content-Type-Options", "nosniff")));
        assert!(HEADERS.contains(&("Cache-Control", "no-cache")));
    }
}

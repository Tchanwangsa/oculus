//! MinerU, in its two shapes. Cloud: `client` speaks the protocol, `ledger`
//! holds the daily quota, `batch` groups documents. `local` is one blocking
//! POST to the user's own server. `render` turns either's content list into
//! markdown.

pub mod batch;
pub mod client;
pub mod ledger;
pub mod local;
pub mod render;

use std::fs;
use std::path::{Path, PathBuf};

use crate::parse::ParseError;

/// A working directory under the system temp dir, removed however the parse
/// ends. `name` must be unique.
struct WorkDir {
    root: PathBuf,
}

impl WorkDir {
    fn new(name: String) -> Result<Self, ParseError> {
        let root = std::env::temp_dir().join(name);
        fs::create_dir_all(&root)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", root.display())))?;
        Ok(Self { root })
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

/// Both engines read the result archive by shape and share its refusal codes.
/// The flat markdown cannot provide the per-page boundaries retrieval needs.
fn content_list(root: &Path) -> Result<(PathBuf, Vec<serde_json::Value>), ParseError> {
    let path = client::find_content_list(root)
        .ok_or(ParseError::Document { code: "no-content-list".into() })?;
    let content: serde_json::Value = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .ok_or(ParseError::Document { code: "unreadable-content-list".into() })?;
    match content {
        serde_json::Value::Array(items) => Ok((path, items)),
        _ => Err(ParseError::Document { code: "invalid-content-list".into() }),
    }
}

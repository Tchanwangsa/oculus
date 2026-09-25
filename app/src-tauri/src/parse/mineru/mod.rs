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

//! Where a notebook's page images live, for chat agents: `canvas-pages/<file id>/`
//! holds one PNG per page and a `pages.json` manifest naming them. The app writes
//! them as the student writes (`library::files::canvases::snapshots`); `oculus read`
//! and the agents read them. Keyed by the `files` row, so a rename moves nothing.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PAGES_DIR: &str = "canvas-pages";
pub const PAGES_MANIFEST: &str = "pages.json";

/// One page's picture, as the manifest records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookPage {
    /// Counted from 1, as the student counts pages.
    pub page: usize,
    /// The page's id in the notebook's `paper`.
    pub id: String,
    /// The PNG's file name inside the notebook's folder.
    pub image: String,
    /// Unix seconds when the picture was last rewritten.
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookPages {
    /// The notebook's library path, `courses/<code>/canvases/<title>.canvas`.
    pub canvas: String,
    pub page_count: usize,
    pub pages: Vec<NotebookPage>,
}

pub fn notebook_pages_dir(data_dir: &Path, file_id: i64) -> PathBuf {
    data_dir.join(PAGES_DIR).join(file_id.to_string())
}

pub fn notebook_page_image(page: usize) -> String {
    format!("page-{page:03}.png")
}

/// The manifest of a notebook's page images, if any have been written.
pub fn read_notebook_pages(data_dir: &Path, file_id: i64) -> Option<NotebookPages> {
    let text =
        std::fs::read_to_string(notebook_pages_dir(data_dir, file_id).join(PAGES_MANIFEST)).ok()?;
    serde_json::from_str(&text).ok()
}

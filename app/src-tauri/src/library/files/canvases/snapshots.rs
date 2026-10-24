//! A notebook's pages as PNGs, for chat agents.
//!
//! The editor renders each page whose content changed a moment after the student
//! stops writing and sends it here. Each lands in `canvas-pages/<file id>/` with a
//! manifest (`library::paths::notebook`), and `oculus read <notebook>` hands the
//! agent those paths. Keyed by file id like the recovery drafts, so a rename moves
//! nothing; a deleted notebook's folder is removed by `canvas_delete_page_snapshots`.

use crate::library::paths::{
    notebook_page_image, read_notebook_pages, NotebookPage, NotebookPages, PAGES_DIR,
    PAGES_MANIFEST,
};
use base64::Engine;
use serde::Deserialize;
use std::path::{Path, PathBuf};

const MAX_PAGES: usize = 500;
const MAX_PAGE_BYTES: usize = 24 * 1024 * 1024;
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// One page picture from the editor: `index` counts from 0, `png` is base64.
#[derive(Deserialize)]
pub struct PageImage {
    id: String,
    index: usize,
    png: String,
}

fn unsafe_folder() -> String {
    "Unsafe notebook page folder".into()
}

/// The notebook's folder, created on demand; a symlink anywhere on the way is refused.
fn dir_at(root: &Path, file_id: i64, create: bool) -> Result<Option<PathBuf>, String> {
    if file_id <= 0 {
        return Err("Invalid canvas file ID".into());
    }
    let mut path = root.to_path_buf();
    for part in [PAGES_DIR.to_string(), file_id.to_string()] {
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                return Err(unsafe_folder())
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && create => {
                std::fs::create_dir(&path).map_err(|e| e.to_string())?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(Some(path))
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = super::super::documents::note_temp_path(path)?;
    crate::runtime::atomic_write::write(path, &tmp, bytes)
}

/// Writes the pictures in `pages` and brings the manifest to `page_count` pages: pages not sent keep
/// their picture and time, pages past the end are deleted. `now` is Unix seconds.
fn write_at(
    root: &Path,
    file_id: i64,
    relative_path: &str,
    page_count: usize,
    pages: Vec<PageImage>,
    now: u64,
) -> Result<(), String> {
    if !super::is_canvas_rel(relative_path) {
        return Err("Expected courses/<subject>/canvases/<title>.canvas".into());
    }
    if page_count == 0 || page_count > MAX_PAGES {
        return Err(format!("A notebook has between 1 and {MAX_PAGES} pages"));
    }
    let mut decoded = Vec::with_capacity(pages.len());
    for page in pages {
        if page.index >= page_count || page.id.is_empty() {
            return Err("A page picture names a page the notebook does not have".into());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(page.png.as_bytes())
            .map_err(|_| "A page picture is not valid base64".to_string())?;
        if bytes.len() > MAX_PAGE_BYTES || !bytes.starts_with(PNG_MAGIC) {
            return Err("A page picture must be a PNG of at most 24 MB".into());
        }
        decoded.push((page.index + 1, page.id, bytes));
    }
    let dir = dir_at(root, file_id, true)?.ok_or_else(unsafe_folder)?;
    let previous = read_notebook_pages(root, file_id);
    let mut manifest: Vec<NotebookPage> = previous
        .map(|m| m.pages)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.page <= page_count)
        .collect();
    for (page, id, bytes) in decoded {
        let image = notebook_page_image(page);
        write_file(&dir.join(&image), &bytes)?;
        manifest.retain(|p| p.page != page);
        manifest.push(NotebookPage {
            page,
            id,
            image,
            updated_at: now,
        });
    }
    manifest.sort_by_key(|p| p.page);
    // A page whose picture was never sent has nothing to show; leave it out rather than list a missing file.
    manifest.retain(|p| dir.join(&p.image).is_file());
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let name = entry.map_err(|e| e.to_string())?.file_name();
        let name = name.to_string_lossy();
        let keep = name == PAGES_MANIFEST || manifest.iter().any(|p| p.image == name);
        if !keep && name.starts_with("page-") && name.ends_with(".png") {
            std::fs::remove_file(dir.join(&*name)).ok();
        }
    }
    let json = serde_json::to_string_pretty(&NotebookPages {
        canvas: relative_path.into(),
        page_count,
        pages: manifest,
    })
    .map_err(|e| e.to_string())?;
    write_file(&dir.join(PAGES_MANIFEST), json.as_bytes())
}

fn remove_at(root: &Path, file_id: i64) -> Result<(), String> {
    match dir_at(root, file_id, false)? {
        Some(dir) => std::fs::remove_dir_all(dir).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[tauri::command]
pub async fn canvas_write_page_snapshots(
    file_id: i64,
    relative_path: String,
    page_count: usize,
    pages: Vec<PageImage>,
) -> Result<(), String> {
    crate::runtime::blocking::run(move || {
        write_at(
            &crate::library::paths::data_dir(),
            file_id,
            &relative_path,
            page_count,
            pages,
            unix_now(),
        )
    })
    .await
}

#[tauri::command]
pub async fn canvas_delete_page_snapshots(file_id: i64) -> Result<(), String> {
    crate::runtime::blocking::run(move || remove_at(&crate::library::paths::data_dir(), file_id))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::paths::notebook_pages_dir;

    const REL: &str = "courses/MAST30027_2026_SM2/canvases/Assignment 1.canvas";

    fn png() -> String {
        let mut bytes = PNG_MAGIC.to_vec();
        bytes.extend_from_slice(b"not a real image, only the signature matters here");
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }
    fn page(index: usize) -> PageImage {
        PageImage {
            id: format!("p{}", index + 1),
            index,
            png: png(),
        }
    }
    fn root() -> crate::test_support::Scratch {
        crate::test_support::Scratch::new("canvas-pages")
    }

    #[test]
    fn writes_each_page_and_a_manifest_the_cli_can_read() {
        let root = root();
        write_at(&root, 7, REL, 2, vec![page(0), page(1)], 100).unwrap();
        let dir = notebook_pages_dir(&root, 7);
        assert!(dir.join("page-001.png").is_file() && dir.join("page-002.png").is_file());
        let manifest = read_notebook_pages(&root, 7).unwrap();
        assert_eq!(manifest.canvas, REL);
        assert_eq!(manifest.page_count, 2);
        assert_eq!(
            manifest
                .pages
                .iter()
                .map(|p| (p.page, p.id.as_str(), p.updated_at))
                .collect::<Vec<_>>(),
            [(1, "p1", 100), (2, "p2", 100)]
        );
    }

    #[test]
    fn rewriting_one_page_leaves_the_others_and_their_times() {
        let root = root();
        write_at(&root, 7, REL, 2, vec![page(0), page(1)], 100).unwrap();
        write_at(&root, 7, REL, 2, vec![page(1)], 250).unwrap();
        let manifest = read_notebook_pages(&root, 7).unwrap();
        assert_eq!(
            manifest
                .pages
                .iter()
                .map(|p| p.updated_at)
                .collect::<Vec<_>>(),
            [100, 250]
        );
    }

    #[test]
    fn a_shorter_notebook_drops_the_pages_past_its_end() {
        let root = root();
        write_at(&root, 7, REL, 3, vec![page(0), page(1), page(2)], 100).unwrap();
        write_at(&root, 7, REL, 1, vec![], 200).unwrap();
        let dir = notebook_pages_dir(&root, 7);
        assert!(dir.join("page-001.png").is_file());
        assert!(!dir.join("page-002.png").exists() && !dir.join("page-003.png").exists());
        let manifest = read_notebook_pages(&root, 7).unwrap();
        assert_eq!((manifest.page_count, manifest.pages.len()), (1, 1));
    }

    #[test]
    fn a_rename_only_rewrites_the_manifest() {
        let root = root();
        write_at(&root, 7, REL, 1, vec![page(0)], 100).unwrap();
        let moved = "courses/MAST30027_2026_SM2/canvases/Renamed.canvas";
        write_at(&root, 7, moved, 1, vec![], 300).unwrap();
        let manifest = read_notebook_pages(&root, 7).unwrap();
        assert_eq!(manifest.canvas, moved);
        assert_eq!(manifest.pages[0].updated_at, 100);
    }

    #[test]
    fn refuses_what_is_not_a_page_of_this_notebook() {
        let root = root();
        let bad_png = PageImage {
            id: "p1".into(),
            index: 0,
            png: base64::engine::general_purpose::STANDARD.encode(b"GIF89a...."),
        };
        assert!(write_at(&root, 7, REL, 1, vec![bad_png], 1).is_err());
        assert!(
            write_at(&root, 7, REL, 1, vec![page(1)], 1).is_err(),
            "index past the page count"
        );
        assert!(write_at(&root, 7, REL, 0, vec![], 1).is_err());
        assert!(write_at(&root, 7, "../elsewhere.canvas", 1, vec![page(0)], 1).is_err());
        assert!(write_at(&root, 0, REL, 1, vec![page(0)], 1).is_err());
        let not_base64 = PageImage {
            id: "p1".into(),
            index: 0,
            png: "***".into(),
        };
        assert!(write_at(&root, 7, REL, 1, vec![not_base64], 1).is_err());
        assert!(
            read_notebook_pages(&root, 7).is_none(),
            "nothing was written"
        );
    }

    #[test]
    #[cfg(unix)]
    fn refuses_a_symlinked_folder() {
        let root = root();
        let elsewhere = crate::test_support::Scratch::new("canvas-pages-elsewhere");
        std::os::unix::fs::symlink(&elsewhere, root.join(PAGES_DIR)).unwrap();
        assert!(write_at(&root, 7, REL, 1, vec![page(0)], 1).is_err());
        assert!(std::fs::read_dir(&elsewhere).unwrap().next().is_none());
    }

    #[test]
    fn deleting_a_notebook_removes_its_folder_and_only_its_own() {
        let root = root();
        write_at(&root, 7, REL, 1, vec![page(0)], 1).unwrap();
        write_at(&root, 8, REL, 1, vec![page(0)], 1).unwrap();
        remove_at(&root, 7).unwrap();
        assert!(read_notebook_pages(&root, 7).is_none());
        assert!(read_notebook_pages(&root, 8).is_some());
        remove_at(&root, 7).unwrap();
    }
}

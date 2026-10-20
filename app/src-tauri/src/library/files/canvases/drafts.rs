//! Desktop recovery records are separate from the saved canvas, keyed by file ID.
use std::path::{Path, PathBuf};

fn path_at(root: &Path, file_id: i64, create: bool) -> Result<Option<PathBuf>, String> {
    if file_id <= 0 {
        return Err("Invalid canvas file ID".into());
    }
    let mut path = root.to_path_buf();
    for part in ["recovery", "canvases"] {
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                return Err("Unsafe canvas recovery folder".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
                std::fs::create_dir(&path).map_err(|e| e.to_string())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        }
    }
    path.push(format!("{file_id}.json"));
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
            return Err("Unsafe canvas recovery file".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    Ok(Some(path))
}

fn read_at(root: &Path, file_id: i64) -> Result<Option<String>, String> {
    let Some(path) = path_at(root, file_id, false)? else {
        return Ok(None);
    };
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn write_at(root: &Path, file_id: i64, content: &str) -> Result<(), String> {
    let path = path_at(root, file_id, true)?.ok_or("Missing recovery folder")?;
    let tmp = super::super::documents::note_temp_path(&path)?;
    crate::runtime::atomic_write::write(&path, &tmp, content.as_bytes())
}

fn remove_at(root: &Path, file_id: i64) -> Result<(), String> {
    let Some(path) = path_at(root, file_id, false)? else {
        return Ok(());
    };
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub async fn read_canvas_draft(file_id: i64) -> Result<Option<String>, String> {
    crate::runtime::blocking::run(move || read_at(&crate::library::paths::data_dir(), file_id))
        .await
}

#[tauri::command]
pub async fn write_canvas_draft(file_id: i64, content: String) -> Result<(), String> {
    crate::runtime::blocking::run(move || {
        write_at(&crate::library::paths::data_dir(), file_id, &content)
    })
    .await
}

#[tauri::command]
pub async fn delete_canvas_draft(file_id: i64) -> Result<(), String> {
    crate::runtime::blocking::run(move || remove_at(&crate::library::paths::data_dir(), file_id))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    #[test]
    fn large_recovery_is_independent_of_the_saved_canvas_and_clears() {
        let root = Scratch::new("canvas-disk-recovery");
        let canvas = root.join("saved.canvas");
        std::fs::write(&canvas, "saved document").unwrap();
        assert_eq!(read_at(&root, 42).unwrap(), None);
        let draft = "x".repeat(6 * 1024 * 1024);
        write_at(&root, 42, &draft).unwrap();
        assert_eq!(read_at(&root, 42).unwrap(), Some(draft));
        assert_eq!(std::fs::read_to_string(canvas).unwrap(), "saved document");
        write_at(&root, 42, "newer draft").unwrap();
        assert_eq!(read_at(&root, 42).unwrap().as_deref(), Some("newer draft"));
        remove_at(&root, 42).unwrap();
        remove_at(&root, 42).unwrap();
        assert_eq!(read_at(&root, 42).unwrap(), None);
        assert!(write_at(&root, -1, "invalid").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn recovery_paths_refuse_symlinks() {
        let root = Scratch::new("canvas-draft-scope");
        let outside = Scratch::new("canvas-draft-outside");
        std::os::unix::fs::symlink(&*outside, root.join("recovery")).unwrap();
        assert!(write_at(&root, 1, "draft").is_err());
        assert!(read_at(&root, 1).is_err());
        assert!(remove_at(&root, 1).is_err());
    }
}

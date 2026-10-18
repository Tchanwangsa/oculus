//! Versioned drawing documents under each subject's own canvases folder.
pub(crate) mod export;

use super::ImportedFile;
use std::path::{Path, PathBuf};

pub(super) fn is_canvas_rel(rel: &str) -> bool {
    let parts: Vec<_> = rel.split('/').collect();
    !rel.contains(['\\', ':'])
        && parts.len() == 4
        && parts[0] == "courses"
        && parts[2] == "canvases"
        && parts
            .iter()
            .all(|p| !p.is_empty() && *p != "." && *p != "..")
        && parts[3].len() > 7
        && parts[3].ends_with(".canvas")
        && !parts[3].starts_with('.')
}

fn name(title: &str) -> String {
    let stem = crate::library::paths::safe_filename(title.trim().trim_end_matches(".canvas"));
    let stem = stem.trim_end_matches('.');
    format!(
        "{}.canvas",
        if stem.trim_matches('_').is_empty() {
            "Untitled"
        } else {
            stem
        }
    )
}

// Reject symlinks even when they point inside the root: a drawing must never
// alias a different library file.
fn checked(root: &Path, rel: &str, create_parents: bool) -> Result<PathBuf, String> {
    if !is_canvas_rel(rel) {
        return Err("Expected courses/<subject>/canvases/<title>.canvas".into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut path = root.clone();
    let parts: Vec<_> = rel.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err("Canvas paths cannot contain symlinks".into())
            }
            Ok(meta) if i < 3 && !meta.is_dir() => {
                return Err("Canvas parent is not a folder".into())
            }
            Ok(meta) if i == 3 && !meta.is_file() => return Err("Canvas is not a file".into()),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && i < 3 && create_parents => {
                std::fs::create_dir(&path).map_err(|e| e.to_string())?;
                let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                if meta.file_type().is_symlink() || !meta.is_dir() {
                    return Err("Unsafe canvas folder".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && i == 3 => {}
            Err(e) => return Err(e.to_string()),
        }
        if i < 3
            && !path
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(&root)
        {
            return Err("Canvas path is outside the library".into());
        }
    }
    Ok(path)
}

fn file(rel: String, size_bytes: u64) -> ImportedFile {
    ImportedFile {
        filename: rel.rsplit('/').next().unwrap_or_default().into(),
        relative_path: rel,
        file_type: "canvas".into(),
        size_bytes,
    }
}

pub(super) fn validate(content: &str) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(content).map_err(|e| e.to_string())?;
    if value.get("type").and_then(|v| v.as_str()) != Some("oculus-canvas")
        || value.get("version").and_then(|v| v.as_u64()) != Some(1)
        || !value.get("elements").is_some_and(|v| v.is_array())
        || !value.get("viewport").is_some_and(|v| v.is_object())
    {
        return Err("Unsupported canvas document".into());
    }
    Ok(())
}

pub(super) fn write_at(root: &Path, rel: &str, content: &str) -> Result<u64, String> {
    validate(content)?;
    let path = checked(root, rel, false)?;
    if !path.is_file() {
        return Err("Canvas no longer exists".into());
    }
    let tmp = super::documents::note_temp_path(&path)?;
    crate::runtime::atomic_write::write(&path, &tmp, content.as_bytes())?;
    Ok(content.len() as u64)
}

pub(super) fn create_at(
    root: &Path,
    code: &str,
    title: &str,
    content: &str,
) -> Result<ImportedFile, String> {
    validate(content)?;
    let prefix = format!("courses/{}/canvases", crate::library::paths::safe_dir(code));
    let wanted = name(title);
    for index in 1.. {
        let candidate = if index == 1 {
            wanted.clone()
        } else {
            format!("{}-{index}.canvas", wanted.trim_end_matches(".canvas"))
        };
        let rel = format!("{prefix}/{candidate}");
        let path = checked(root, &rel, true)?;
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
        {
            Ok(_) => {
                if let Err(error) = write_at(root, &rel, content) {
                    std::fs::remove_file(path).ok();
                    return Err(error);
                }
                return Ok(file(rel, content.len() as u64));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    unreachable!()
}

#[tauri::command]
pub async fn create_canvas(
    subject_code: String,
    title: String,
    content: String,
) -> Result<ImportedFile, String> {
    crate::runtime::blocking::run(move || {
        create_at(
            &crate::library::paths::data_dir(),
            &subject_code,
            &title,
            &content,
        )
    })
    .await
}

#[tauri::command]
pub async fn read_canvas(relative_path: String) -> Result<String, String> {
    crate::runtime::blocking::run(move || {
        std::fs::read_to_string(checked(
            &crate::library::paths::data_dir(),
            &relative_path,
            false,
        )?)
        .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub async fn write_canvas(relative_path: String, content: String) -> Result<u64, String> {
    crate::runtime::blocking::run(move || {
        write_at(&crate::library::paths::data_dir(), &relative_path, &content)
    })
    .await
}

#[tauri::command]
pub async fn rename_canvas(relative_path: String, title: String) -> Result<ImportedFile, String> {
    crate::runtime::blocking::run(move || {
        rename_at(&crate::library::paths::data_dir(), &relative_path, &title)
    })
    .await
}

pub(super) fn rename_at(root: &Path, rel: &str, title: &str) -> Result<ImportedFile, String> {
    let path = checked(root, rel, false)?;
    let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    let (prefix, current) = rel.rsplit_once('/').ok_or("Missing filename")?;
    let wanted = name(title);
    if wanted == current {
        return Ok(file(rel.into(), size));
    }
    let destination =
        super::uploads::step_aside(path.parent().ok_or("Missing parent")?, &wanted, |_| false);
    let moved = format!("{prefix}/{destination}");
    let target = checked(root, &moved, false)?;
    std::fs::rename(path, target).map_err(|e| e.to_string())?;
    Ok(file(moved, size))
}

#[tauri::command]
pub async fn delete_canvas(relative_path: String) -> Result<(), String> {
    crate::runtime::blocking::run(move || {
        std::fs::remove_file(checked(
            &crate::library::paths::data_dir(),
            &relative_path,
            false,
        )?)
        .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub async fn list_canvases(subject_code: String) -> Result<Vec<ImportedFile>, String> {
    crate::runtime::blocking::run(move || {
        list_at(&crate::library::paths::data_dir(), &subject_code)
    })
    .await
}

pub(super) fn list_at(root: &Path, code: &str) -> Result<Vec<ImportedFile>, String> {
    let prefix = format!("courses/{}/canvases", crate::library::paths::safe_dir(code));
    let probe = match checked(root, &format!("{prefix}/Untitled.canvas"), false) {
        Ok(path) => path,
        Err(_) if !root.join(&prefix).exists() => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut files = Vec::new();
    for entry in
        std::fs::read_dir(probe.parent().ok_or("Missing parent")?).map_err(|e| e.to_string())?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let rel = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        if !is_canvas_rel(&rel) {
            continue;
        }
        let path = checked(root, &rel, false)?;
        files.push(file(
            rel,
            std::fs::metadata(path).map_err(|e| e.to_string())?.len(),
        ));
    }
    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(files)
}

#[cfg(test)]
mod tests;

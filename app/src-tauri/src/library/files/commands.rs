use std::path::Path;

#[tauri::command]
pub async fn read_course_file(relative_path: String) -> Result<String, String> {
    crate::runtime::blocking::run(move || {
        let path = crate::library::paths::data_dir().join(&relative_path);
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    })
    .await
}

pub(super) fn file_has_content(root: &Path, relative_path: &str) -> Result<bool, String> {
    if relative_path.is_empty()
        || Path::new(relative_path)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("expected a path relative to the library".into());
    }
    let path = match root.join(relative_path).canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    if !path.starts_with(&root) {
        return Err("path is outside the library".into());
    }
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    Ok(metadata.is_file() && metadata.len() > 0)
}

/// Check a parsed sibling's presence without transferring its markdown.
#[tauri::command]
pub async fn course_file_has_content(relative_path: String) -> Result<bool, String> {
    crate::runtime::blocking::run(move || {
        file_has_content(&crate::library::paths::data_dir(), &relative_path)
    })
    .await
}

#[tauri::command]
pub fn open_course_file(relative_path: String) -> Result<(), String> {
    let path = crate::library::paths::data_dir().join(&relative_path);
    tauri_plugin_opener::open_path(path.to_str().unwrap_or(""), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Derive parse status from disk for a set of PDF-backed relative paths
/// (PDFs, plus Office files parsed via their derived sibling PDF).
/// Returns (relative_path, status); paths with no parse output are omitted.
#[tauri::command]
pub async fn scan_parsed_files(
    relative_paths: Vec<String>,
) -> Result<Vec<(String, String)>, String> {
    crate::runtime::blocking::run(move || {
        let base = crate::library::paths::data_dir();
        Ok(relative_paths
            .into_iter()
            .filter_map(|rel| {
                let pdf_rel = crate::library::paths::doc_pdf_rel(&rel)?;
                crate::parse::parse_mode(&base.join(&pdf_rel)).map(|mode| (rel, mode.to_string()))
            })
            .collect())
    })
    .await
}

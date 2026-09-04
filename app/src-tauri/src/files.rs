use tauri::{AppHandle, Manager};

/// Live cookies from the login WebView (only available while it's open).
pub fn canvas_cookie_header(app: &AppHandle) -> String {
    let Some(win) = app.get_webview_window("canvas-auth") else {
        return String::new();
    };
    match win.cookies() {
        Ok(cookies) => cookies
            .iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect::<Vec<_>>()
            .join("; "),
        Err(e) => {
            eprintln!("[oculus] cookies() failed: {e}");
            String::new()
        }
    }
}

/// Cookie to use for server-side Canvas requests. Prefers the persisted
/// snapshot (survives restart); falls back to the live login WebView if it
/// happens to be open and nothing was saved yet.
pub fn proxy_cookie(app: &AppHandle) -> String {
    let saved = crate::auth::saved_cookie_header(app);
    if !saved.is_empty() {
        return saved;
    }
    canvas_cookie_header(app)
}

/// A library file's markdown, resolved headlessly (no `AppHandle`, so the
/// agent and the CLI can call it).
///
/// Markdown-native files — Canvas pages, announcements, Ed threads — *are*
/// the markdown. PDF-backed ones (real PDFs and the Office conversions) have
/// it beside the PDF as `{stem}.md`, written by the parser; a file that has
/// not been parsed yet has none, which is a meaningful answer rather than an
/// error the caller should retry.
pub fn read_parsed_markdown(relative_path: &str) -> Result<String, String> {
    let base = crate::paths::data_dir();
    if relative_path.to_ascii_lowercase().ends_with(".md") {
        return std::fs::read_to_string(base.join(relative_path)).map_err(|e| e.to_string());
    }
    let pdf_rel = crate::paths::doc_pdf_rel(relative_path)
        .ok_or_else(|| format!("{relative_path}: not a document with parsed markdown"))?;
    let md = base.join(&pdf_rel).with_extension("md");
    if !md.is_file() {
        return Err("not parsed yet — no markdown on disk".into());
    }
    std::fs::read_to_string(md).map_err(|e| e.to_string())
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn read_course_file(app: AppHandle, relative_path: String) -> Result<String, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(&relative_path);
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_course_file(app: AppHandle, relative_path: String) -> Result<(), String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(&relative_path);
    tauri_plugin_opener::open_path(path.to_str().unwrap_or(""), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Derive parse status from disk for a set of PDF-backed relative paths
/// (PDFs, plus Office files parsed via their derived sibling PDF).
/// Returns (relative_path, status); paths with no parse output are omitted.
#[tauri::command]
pub fn scan_parsed_files(
    app: AppHandle,
    relative_paths: Vec<String>,
) -> Result<Vec<(String, String)>, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(relative_paths
        .into_iter()
        .filter_map(|rel| {
            let pdf_rel = crate::paths::doc_pdf_rel(&rel)?;
            crate::paths::parse_mode(&base.join(&pdf_rel)).map(|mode| (rel, mode.to_string()))
        })
        .collect())
}

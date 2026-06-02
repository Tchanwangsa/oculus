use tauri::AppHandle;

pub fn safe_dir(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

pub fn safe_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect::<String>()
        .replace("..", "_")
}

pub fn safe_rel_path(rel: &str) -> Option<String> {
    let parts: Vec<String> = rel
        .split('/')
        .filter(|s| !s.is_empty())
        .map(safe_filename)
        .filter(|s| s != "." && s != "_" && !s.is_empty())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

pub fn write_course_bytes(
    app: &AppHandle,
    code: &str,
    rel_path: &str,
    content: &[u8],
) -> Result<(String, u64), String> {
    let safe = safe_rel_path(rel_path).ok_or_else(|| format!("invalid path: {rel_path}"))?;
    let rel = format!("courses/{}/{}", safe_dir(code), safe);
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(&rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok((rel, content.len() as u64))
}

pub fn parse_query(url: &str) -> std::collections::HashMap<String, String> {
    match url::Url::parse(&format!("http://x{url}")) {
        Ok(u) => u.query_pairs().into_owned().collect(),
        Err(_) => std::collections::HashMap::new(),
    }
}

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

pub fn category_from_path(path: &str) -> &'static str {
    if path == "home.md" {
        "home"
    } else if path == "syllabus.md" {
        "syllabus"
    } else if path.starts_with("pages/") {
        "page"
    } else if path.starts_with("assignments/") {
        "assignment"
    } else if path.starts_with("announcements/") {
        "announcement"
    } else if path.starts_with("files/") {
        "file"
    } else if path.starts_with("modules/") {
        "module"
    } else if path.starts_with("images/") {
        "image"
    } else {
        "other"
    }
}

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

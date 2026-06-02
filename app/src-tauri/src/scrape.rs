use std::io::Read as _;
use tauri::{AppHandle, Emitter, Manager};

use crate::auth::{auth_flag_path, AuthState};
use crate::files::{proxy_cookie, write_course_bytes};
use crate::ipc::IpcPort;
use crate::worker::{ensure_worker_window, WORKER_LABEL};

#[derive(serde::Deserialize)]
pub struct ScrapeSubject {
    pub id: i64,
    pub code: String,
}

const SCRAPER_JS: &str = include_str!("../scraper.js");

#[tauri::command]
pub fn cancel_scrape(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(WORKER_LABEL) {
        win.eval("window.__oculus_cancel = true;")
            .map_err(|e| e.to_string())?;
        eprintln!("[oculus] cancel_scrape: signalled");
    }
    Ok(())
}

#[tauri::command]
pub async fn scrape_content(
    app: AppHandle,
    subjects: Vec<ScrapeSubject>,
    port: tauri::State<'_, IpcPort>,
    auth: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    if subjects.is_empty() {
        return Err("No subjects selected.".to_string());
    }

    if !auth_flag_path(&app).exists() {
        *auth.0.lock().unwrap() = false;
        app.emit("canvas-auth-expired", "not-authenticated").ok();
        return Err("Not authenticated. Connect to Canvas first.".to_string());
    }

    // Scraper JS runs in the hidden worker WebView; all Canvas fetches go
    // through the cookie proxy, so no logged-in WebView is required.
    ensure_worker_window(&app, port.0);
    let win = app
        .get_webview_window(WORKER_LABEL)
        .ok_or_else(|| "Worker window unavailable".to_string())?;

    let subjects_json = serde_json::to_string(
        &subjects
            .iter()
            .map(|s| serde_json::json!({ "id": s.id, "code": s.code }))
            .collect::<Vec<_>>(),
    )
    .map_err(|e| e.to_string())?;

    let js = SCRAPER_JS
        .replace("__PORT__", &port.0.to_string())
        .replace("__SUBJECTS__", &subjects_json);

    eprintln!(
        "[oculus] scrape_content: {} subjects, IPC port={}",
        subjects.len(),
        port.0
    );
    win.eval(&js).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn rescrape_file(
    app: AppHandle,
    subject_id: i64,
    subject_code: String,
    canvas_id: i64,
) -> Result<String, String> {
    const CANVAS_BASE: &str = "https://canvas.lms.unimelb.edu.au";
    const DOWNLOADABLE: &[&str] = &[
        "application/pdf",
        "application/vnd.ms-powerpoint",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "application/msword",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ];

    let cookie = proxy_cookie(&app);
    if cookie.is_empty() {
        return Err("Not authenticated — connect to Canvas first.".to_string());
    }

    let info: serde_json::Value = serde_json::from_reader(
        ureq::get(&format!("{CANVAS_BASE}/api/v1/files/{canvas_id}"))
            .set("Cookie", &cookie)
            .call()
            .map_err(|e| format!("Canvas API: {e}"))?
            .into_reader(),
    )
    .map_err(|e| e.to_string())?;

    let ct = info
        .get("content-type")
        .or_else(|| info.get("content_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();

    if !DOWNLOADABLE.contains(&ct) {
        return Err(format!("File type '{ct}' not in download allowlist"));
    }

    let pub_info: serde_json::Value = serde_json::from_reader(
        ureq::get(&format!("{CANVAS_BASE}/api/v1/files/{canvas_id}/public_url"))
            .set("Cookie", &cookie)
            .call()
            .map_err(|e| format!("public_url API: {e}"))?
            .into_reader(),
    )
    .map_err(|e| e.to_string())?;

    let dl_url = pub_info["public_url"]
        .as_str()
        .or_else(|| info["url"].as_str())
        .ok_or_else(|| "No download URL in API response".to_string())?
        .to_string();

    let name = info["filename"]
        .as_str()
        .or_else(|| info["display_name"].as_str())
        .unwrap_or("file.bin")
        .replace(['/', '\\'], "_");

    let mut file_bytes: Vec<u8> = Vec::new();
    ureq::get(&dl_url)
        .call()
        .map_err(|e| format!("download: {e}"))?
        .into_reader()
        .read_to_end(&mut file_bytes)
        .map_err(|e| e.to_string())?;

    let path = format!("files/{name}");
    let (rel, size_saved) = write_course_bytes(&app, &subject_code, &path, &file_bytes)?;

    app.emit(
        "scrape-file",
        serde_json::json!({
            "subject_id": subject_id,
            "code": subject_code,
            "relative_path": rel,
            "size_bytes": size_saved,
            "category": "file",
            "canvas_id": canvas_id,
        }),
    )
    .ok();

    eprintln!("[oculus] rescrape_file: saved {rel} ({size_saved} bytes)");
    Ok(rel)
}

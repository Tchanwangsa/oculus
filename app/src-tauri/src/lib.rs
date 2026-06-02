use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub struct AuthState(pub Arc<Mutex<bool>>);
pub struct SubjectsState(pub Arc<Mutex<Vec<serde_json::Value>>>);
pub struct IpcPort(pub u16);

fn canvas_session_dir(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .expect("no app data dir")
        .join("canvas-session")
}

fn auth_flag_path(app: &AppHandle) -> std::path::PathBuf {
    canvas_session_dir(app).join("authenticated")
}

fn cors_header(key: &[u8], val: &[u8]) -> tiny_http::Header {
    tiny_http::Header::from_bytes(key, val).unwrap()
}

fn cors_response(status: u16) -> tiny_http::Response<std::io::Empty> {
    tiny_http::Response::empty(status)
        .with_header(cors_header(b"Access-Control-Allow-Origin", b"*"))
        .with_header(cors_header(b"Access-Control-Allow-Methods", b"POST, OPTIONS"))
        .with_header(cors_header(b"Access-Control-Allow-Headers", b"Content-Type"))
}

/// True only when the URL is a Canvas page that requires an authenticated session.
/// `/login*` is NOT included — it's a transient hop in the SSO redirect chain.
fn is_authenticated_url(url: &url::Url) -> bool {
    url.host_str() == Some("canvas.lms.unimelb.edu.au")
        && {
            let p = url.path();
            p == "/"
                || p.starts_with("/dashboard")
                || p.starts_with("/courses")
                || p.starts_with("/calendar")
                || p.starts_with("/inbox")
        }
}

/// Shared window-creation logic used by both startup restore and user-initiated auth.
///
/// `silent` = true  → hidden window, navigate to /login/saml to trigger SSO.
///                     If the SSO session cookie is still valid the IdP auto-redirects
///                     back to the dashboard (authenticated). If not, the window sits on
///                     the SSO login form. A timeout thread decides success vs expiry —
///                     we never react to a transient `/login` navigation.
/// `silent` = false → visible window for interactive login.
fn open_canvas_window(app: AppHandle, auth_flag: Arc<Mutex<bool>>, silent: bool) {
    if let Some(existing) = app.get_webview_window("canvas-auth") {
        existing.close().ok();
        std::thread::sleep(std::time::Duration::from_millis(120));
    }

    // Both modes start at the SSO entry point so existing cookies get a chance to auth.
    let url = "https://canvas.lms.unimelb.edu.au/login/saml";

    let session_dir   = canvas_session_dir(&app);
    let flag_path     = auth_flag_path(&app);
    let app_nav       = app.clone();
    let app_win       = app.clone();
    let auth_flag_nav = Arc::clone(&auth_flag);
    let auth_flag_win = Arc::clone(&auth_flag);

    // Per-attempt resolution flag — distinguishes "this restore succeeded" from global state.
    let resolved = Arc::new(AtomicBool::new(false));
    let resolved_nav = Arc::clone(&resolved);

    let result = WebviewWindowBuilder::new(
        &app,
        "canvas-auth",
        WebviewUrl::External(url.parse().unwrap()),
    )
    .title("Sign in to Canvas — Oculus")
    .inner_size(900.0, 700.0)
    .center()
    .visible(!silent)
    .data_directory(session_dir)
    .on_navigation(move |url| {
        if is_authenticated_url(&url) {
            // Reached an authenticated Canvas page — success (both modes).
            let was_resolved = resolved_nav.swap(true, Ordering::SeqCst);
            if !was_resolved {
                *auth_flag_nav.lock().unwrap() = true;
                std::fs::create_dir_all(flag_path.parent().unwrap()).ok();
                std::fs::write(&flag_path, b"1").ok();

                if let Some(w) = app_nav.get_webview_window("canvas-auth") {
                    w.hide().ok();
                }
                app_nav.emit("canvas-auth-success", "ok").ok();
                eprintln!("[oculus] auth success ({})", if silent { "silent" } else { "interactive" });
            }
        }
        true
    })
    .build();

    let win = match result {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[oculus] failed to open canvas window: {e}");
            return;
        }
    };

    if silent {
        // Timeout: give the SSO redirect chain time to complete. If we haven't reached an
        // authenticated page by then, the session is genuinely expired — clean up to a
        // disconnected state so the UI is unambiguous.
        let app_to = app.clone();
        let flag_path_to = auth_flag_path(&app);
        let session_dir_to = canvas_session_dir(&app);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(15));
            if !resolved.load(Ordering::SeqCst) {
                eprintln!("[oculus] silent restore timed out — session expired, resetting");
                *auth_flag_win.lock().unwrap() = false;
                if let Some(w) = app_to.get_webview_window("canvas-auth") {
                    w.close().ok();
                }
                // Clear stale cookies + flag so state is clean: user must reconnect
                std::fs::remove_dir_all(&session_dir_to).ok();
                let _ = &flag_path_to; // (removed with the dir above)
                app_to.emit("canvas-auth-expired", "expired").ok();
            }
        });
    } else {
        // Interactive: detect user closing the window before completing login.
        win.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if !*auth_flag_win.lock().unwrap() {
                    app_win.emit("canvas-auth-cancelled", "cancelled").ok();
                }
            }
        });
    }
}

#[tauri::command]
fn get_auth_status(app: AppHandle, state: tauri::State<AuthState>) -> bool {
    let flag = auth_flag_path(&app);
    eprintln!("[oculus] get_auth_status — flag path: {}", flag.display());
    let file_says_auth = flag.exists();
    let mem_says_auth  = *state.0.lock().unwrap();
    eprintln!("[oculus] get_auth_status — file={file_says_auth} mem={mem_says_auth}");

    if file_says_auth && !mem_says_auth {
        // Startup race: file exists but memory not yet set — fix it now
        *state.0.lock().unwrap() = true;
    }

    file_says_auth || mem_says_auth
}

#[tauri::command]
fn get_subjects(state: tauri::State<SubjectsState>) -> Vec<serde_json::Value> {
    state.0.lock().unwrap().clone()
}

#[tauri::command]
fn open_canvas_devtools(app: AppHandle) {
    if let Some(win) = app.get_webview_window("canvas-auth") {
        win.show().ok();
        win.open_devtools();
    }
}

#[tauri::command]
async fn launch_canvas_auth(
    app: AppHandle,
    state: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    let auth_flag = Arc::clone(&state.0);
    open_canvas_window(app, auth_flag, false);
    Ok(())
}

#[tauri::command]
async fn disconnect_canvas(
    app: AppHandle,
    state: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    *state.0.lock().unwrap() = false;

    if let Some(win) = app.get_webview_window("canvas-auth") {
        win.close().map_err(|e| e.to_string())?;
    }

    // Delete entire session dir → clears cookies + auth flag
    let session_dir = canvas_session_dir(&app);
    if session_dir.exists() {
        std::fs::remove_dir_all(&session_dir).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
async fn sync_subjects(
    app: AppHandle,
    port: tauri::State<'_, IpcPort>,
    auth: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    let win = match app.get_webview_window("canvas-auth") {
        Some(w) => w,
        None => {
            // No live Canvas window — session not ready. Reset UI to disconnected.
            *auth.0.lock().unwrap() = false;
            app.emit("canvas-auth-expired", "window-missing").ok();
            return Err("Canvas session not ready. Click Connect to Canvas.".to_string());
        }
    };

    let p = port.0;
    eprintln!("[oculus] evaling sync_subjects, IPC port={p}");

    win.eval(&format!(r#"
(async () => {{
    console.log('[Oculus] sync_subjects started, IPC port={p}');

    const post = async (path, body) => {{
        const resp = await fetch(`http://127.0.0.1:{p}${{path}}`, {{
            method: 'POST',
            headers: {{ 'Content-Type': 'application/json' }},
            body: typeof body === 'string' ? body : JSON.stringify(body),
        }});
        console.log('[Oculus] POST', path, '->', resp.status);
    }};

    try {{
        const resp = await fetch(
            '/api/v1/courses?per_page=100&include[]=term&include[]=account',
            {{ credentials: 'include' }}
        );
        if (!resp.ok) throw new Error(`Canvas API ${{resp.status}}`);
        const all = await resp.json();

        // Keep only real academic courses — filter out:
        //   Default Term  = admin/community groups
        //   MPMP prefix   = Melbourne Peer Mentor Program (not a subject)
        const NON_SUBJECT_PREFIXES = ['MPMP'];
        const academic = all.filter(c =>
            c.term &&
            c.term.name !== 'Default Term' &&
            (c.workflow_state === 'available' || c.workflow_state === 'completed') &&
            !NON_SUBJECT_PREFIXES.some(p => (c.course_code || '').startsWith(p))
        );

        // Find the latest term among available (enrolled) courses.
        // UniMelb format "YYYY Semester N" sorts correctly lexicographically.
        // enrollment_state=active is NOT reliable — keeps old semesters active too.
        const availableTerms = academic
            .filter(c => c.workflow_state === 'available' && c.term?.name)
            .map(c => c.term.name);
        const latestTerm = [...new Set(availableTerms)].sort().reverse()[0];
        console.log('[Oculus] latest term detected:', latestTerm);

        // Only courses in the latest term are "current" — everything else is past
        const courses = academic.map(c => ({{
            ...c,
            _oculus_is_current: c.term?.name === latestTerm && c.workflow_state === 'available',
        }}));

        const nCurrent = courses.filter(c => c._oculus_is_current).length;
        const nPast    = courses.length - nCurrent;
        console.log(`[Oculus] current: ${{nCurrent}} | past: ${{nPast}} | total: ${{courses.length}}`);

        if (courses.length === 0) throw new Error('No courses — session may have expired. Re-authenticate.');

        await post('/subjects', courses);
        console.log('[Oculus] done');
    }} catch (err) {{
        console.error('[Oculus] error:', err);
        try {{ await post('/error', String(err)); }} catch {{}}
    }}
}})();
    "#))
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(serde::Deserialize)]
struct ScrapeSubject {
    id: i64,
    code: String,
}

/// Sanitize a course code into a filesystem-safe directory name.
fn safe_dir(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// Sanitize a filename — keeps dots (extension) but blocks path traversal / separators.
fn safe_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect::<String>()
        .replace("..", "_")
}

/// Sanitize a relative path (may contain `/`): each segment cleaned, traversal blocked.
fn safe_rel_path(rel: &str) -> Option<String> {
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

/// Write raw bytes under {app_data}/courses/{code}/{rel_path}. Returns (relative_path, bytes).
fn write_course_bytes(
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

/// Parse a `?a=b&c=d` query string off a request URL into a map.
fn parse_query(url: &str) -> std::collections::HashMap<String, String> {
    match url::Url::parse(&format!("http://x{url}")) {
        Ok(u) => u.query_pairs().into_owned().collect(),
        Err(_) => std::collections::HashMap::new(),
    }
}

/// Build a Cookie header from the authenticated canvas-auth WebView's cookies.
/// Lets server-side ureq requests authenticate like the browser session does.
fn canvas_cookie_header(app: &AppHandle) -> String {
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

/// File category inferred from the relative path prefix.
fn category_from_path(path: &str) -> &'static str {
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

/// Read a previously-scraped file back as text (for the markdown viewer).
#[tauri::command]
fn read_course_file(app: AppHandle, relative_path: String) -> Result<String, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(&relative_path);
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

/// Open a scraped file with the system default application (PDF viewer, etc).
#[tauri::command]
fn open_course_file(app: AppHandle, relative_path: String) -> Result<(), String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(&relative_path);
    tauri_plugin_opener::open_path(path.to_str().unwrap_or(""), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Re-download a single Canvas file by canvas_id directly from Rust.
/// Uses the authenticated WebView's cookies so no JS injection needed.
#[tauri::command]
fn rescrape_file(
    app: AppHandle,
    subject_id: i64,
    subject_code: String,
    canvas_id: i64,
) -> Result<String, String> {
    use std::io::Read as _;

    const CANVAS_BASE: &str = "https://canvas.lms.unimelb.edu.au";
    const DOWNLOADABLE: &[&str] = &[
        "application/pdf",
        "application/vnd.ms-powerpoint",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "application/msword",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ];

    let cookie = canvas_cookie_header(&app);
    if cookie.is_empty() {
        return Err("Not authenticated — connect to Canvas first.".to_string());
    }

    let info: serde_json::Value = serde_json::from_reader(
        ureq::get(&format!("{CANVAS_BASE}/api/v1/files/{canvas_id}"))
            .set("Cookie", &cookie)
            .call()
            .map_err(|e| format!("Canvas API: {e}"))?
            .into_reader(),
    ).map_err(|e| e.to_string())?;

    let ct = info.get("content-type")
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
    ).map_err(|e| e.to_string())?;

    let dl_url = pub_info["public_url"].as_str()
        .or_else(|| info["url"].as_str())
        .ok_or_else(|| "No download URL in API response".to_string())?
        .to_string();

    let name = info["display_name"].as_str()
        .or_else(|| info["filename"].as_str())
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

    app.emit("scrape-file", serde_json::json!({
        "subject_id": subject_id,
        "code": subject_code,
        "relative_path": rel,
        "size_bytes": size_saved,
        "category": "file",
        "canvas_id": canvas_id,
    })).ok();

    eprintln!("[oculus] rescrape_file: saved {rel} ({size_saved} bytes)");
    Ok(rel)
}

/// Signal the running scrape agent to stop after the current item.
/// Sets window.__oculus_cancel = true in the Canvas WebView.
#[tauri::command]
fn cancel_scrape(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("canvas-auth") {
        win.eval("window.__oculus_cancel = true;").map_err(|e| e.to_string())?;
        eprintln!("[oculus] cancel_scrape: signalled");
    }
    Ok(())
}

/// Scraper agent JS, injected into the authenticated Canvas WebView.
/// Tokens __PORT__ and __SUBJECTS__ are substituted before eval.
const SCRAPER_JS: &str = include_str!("../scraper.js");

#[tauri::command]
async fn scrape_content(
    app: AppHandle,
    subjects: Vec<ScrapeSubject>,
    port: tauri::State<'_, IpcPort>,
    auth: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    let win = match app.get_webview_window("canvas-auth") {
        Some(w) => w,
        None => {
            *auth.0.lock().unwrap() = false;
            app.emit("canvas-auth-expired", "window-missing").ok();
            return Err("Canvas session not ready. Click Connect to Canvas.".to_string());
        }
    };

    if subjects.is_empty() {
        return Err("No subjects selected.".to_string());
    }

    // Verify auth flag still set before starting a potentially long scrape.
    if !auth_flag_path(&app).exists() {
        *auth.0.lock().unwrap() = false;
        app.emit("canvas-auth-expired", "not-authenticated").ok();
        return Err("Not authenticated. Connect to Canvas first.".to_string());
    }

    let subjects_json = serde_json::to_string(&subjects.iter().map(|s| {
        serde_json::json!({ "id": s.id, "code": s.code })
    }).collect::<Vec<_>>())
    .map_err(|e| e.to_string())?;

    let js = SCRAPER_JS
        .replace("__PORT__", &port.0.to_string())
        .replace("__SUBJECTS__", &subjects_json);

    eprintln!("[oculus] scrape_content: {} subjects, IPC port={}", subjects.len(), port.0);
    win.eval(&js).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AuthState(Arc::new(Mutex::new(false))))
        .manage(SubjectsState(Arc::new(Mutex::new(vec![]))))
        .setup(|app| {
            // ── IPC HTTP server ────────────────────────────────────────
            let server = tiny_http::Server::http("127.0.0.1:0")
                .expect("[oculus] failed to start IPC HTTP server");
            let port = server
                .server_addr()
                .to_ip()
                .expect("IPC server addr missing")
                .port();

            eprintln!("[oculus] IPC HTTP server on 127.0.0.1:{port}");
            app.manage(IpcPort(port));

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                for mut request in server.incoming_requests() {
                    let method = request.method().clone();
                    let url    = request.url().to_string();

                    if method == tiny_http::Method::Options {
                        let _ = request.respond(cors_response(200));
                        continue;
                    }

                    if method == tiny_http::Method::Post {
                        // Read raw bytes once — binary endpoints need them; text decodes lossy.
                        let mut bytes: Vec<u8> = Vec::new();
                        let _ = request.as_reader().read_to_end(&mut bytes);
                        eprintln!("[oculus] IPC POST {url} len={}", bytes.len());

                        // Helper: write a scraped artifact + emit scrape-file event.
                        let emit_file = |code: &str, sid: i64, path: &str, data: &[u8], canvas_id: Option<i64>| {
                            match write_course_bytes(&handle, code, path, data) {
                                Ok((rel, n)) => {
                                    eprintln!("[oculus] wrote {rel} ({n} bytes)");
                                    handle.emit("scrape-file", serde_json::json!({
                                        "subject_id": sid,
                                        "code": code,
                                        "relative_path": rel,
                                        "size_bytes": n,
                                        "category": category_from_path(path),
                                        "canvas_id": canvas_id,
                                    })).ok();
                                }
                                Err(e) => {
                                    eprintln!("[oculus] write failed: {e}");
                                    handle.emit("scrape-error", e).ok();
                                }
                            }
                        };

                        if url.starts_with("/subjects") {
                            let body = String::from_utf8_lossy(&bytes);
                            match serde_json::from_str::<Vec<serde_json::Value>>(&body) {
                                Ok(courses) => {
                                    eprintln!("[oculus] parsed {} courses", courses.len());
                                    *handle.state::<SubjectsState>().0.lock().unwrap() =
                                        courses.clone();
                                    handle.emit("subjects-loaded", courses).ok();
                                }
                                Err(e) => {
                                    eprintln!("[oculus] parse error: {e}");
                                    handle.emit("subjects-error", e.to_string()).ok();
                                }
                            }
                        } else if url.starts_with("/scrape-progress") {
                            let body = String::from_utf8_lossy(&bytes);
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                                handle.emit("scrape-progress", v).ok();
                            }
                        } else if url.starts_with("/scrape-done") {
                            let body = String::from_utf8_lossy(&bytes);
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                                handle.emit("scrape-complete", v).ok();
                            }
                        } else if url.starts_with("/scrape-log") {
                            let body = String::from_utf8_lossy(&bytes);
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                                handle.emit("scrape-log", v).ok();
                            }
                        } else if url.starts_with("/image-proxy") {
                            // JS POSTs the pre-signed CDN URL as plain text; Rust fetches
                            // server-side (no CORS) and saves bytes directly.
                            let qp = parse_query(&url);
                            let code = qp.get("course").map(String::as_str).unwrap_or("UNKNOWN");
                            let sid = qp.get("subject_id").and_then(|s| s.parse().ok()).unwrap_or(0);
                            let path = qp.get("path").map(String::as_str).unwrap_or("");
                            let cid = qp.get("canvas_id").and_then(|s| s.parse().ok());
                            let cdn_url = String::from_utf8_lossy(&bytes).trim().to_string();
                            if path.is_empty() || cdn_url.is_empty() {
                                handle.emit("scrape-error", "image-proxy: missing path or url").ok();
                            } else {
                                let cookie_header = canvas_cookie_header(&handle);
                                let mut req = ureq::get(&cdn_url);
                                if !cookie_header.is_empty() {
                                    req = req.set("Cookie", &cookie_header);
                                }
                                match req.call() {
                                    Ok(resp) => {
                                        let resp_ct = resp.content_type().to_string();
                                        let mut img_bytes: Vec<u8> = Vec::new();
                                        let _ = resp.into_reader().read_to_end(&mut img_bytes);
                                        if resp_ct.contains("text/html") {
                                            // Got a login/error HTML page instead of the image.
                                            eprintln!("[oculus] image-proxy: HTML response (auth failed) for {path}");
                                            handle.emit("scrape-log", serde_json::json!({
                                                "level": "warning", "course": code,
                                                "message": format!("image {path}: got HTML (not image) — auth/url issue")
                                            })).ok();
                                        } else {
                                            eprintln!("[oculus] image-proxy: {} bytes ({resp_ct}) for {}", img_bytes.len(), path);
                                            emit_file(code, sid, path, &img_bytes, cid);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[oculus] image-proxy fetch failed: {e}");
                                        handle.emit("scrape-log", serde_json::json!({
                                            "level": "warning", "course": code,
                                            "message": format!("image-proxy: {e}")
                                        })).ok();
                                    }
                                }
                            }
                        } else if url.starts_with("/scrape-binary") {
                            let qp = parse_query(&url);
                            let code = qp.get("course").map(String::as_str).unwrap_or("UNKNOWN");
                            let sid = qp.get("subject_id").and_then(|s| s.parse().ok()).unwrap_or(0);
                            let path = qp.get("path").map(String::as_str).unwrap_or("");
                            let cid = qp.get("canvas_id").and_then(|s| s.parse().ok());
                            if path.is_empty() {
                                handle.emit("scrape-error", "binary: missing path").ok();
                            } else {
                                emit_file(code, sid, path, &bytes, cid);
                            }
                        } else if url.starts_with("/scrape") {
                            // Text/markdown artifact — metadata in query, body is the markdown.
                            let qp = parse_query(&url);
                            let code = qp.get("course").map(String::as_str).unwrap_or("UNKNOWN");
                            let sid = qp.get("subject_id").and_then(|s| s.parse().ok()).unwrap_or(0);
                            let path = qp.get("path").map(String::as_str).unwrap_or("");
                            if path.is_empty() {
                                handle.emit("scrape-error", "scrape: missing path").ok();
                            } else {
                                emit_file(code, sid, path, &bytes, None);
                            }
                        } else if url.starts_with("/error") {
                            let body = String::from_utf8_lossy(&bytes);
                            eprintln!("[oculus] canvas error: {body}");
                            handle.emit("subjects-error", body.to_string()).ok();
                        }

                        let _ = request.respond(cors_response(200));
                    }
                }
            });

            // ── Silent auth restore on startup ─────────────────────────
            let app_handle = app.handle().clone();
            let flag       = auth_flag_path(&app_handle);

            if flag.exists() {
                eprintln!("[oculus] auth flag found at {} — restoring session", flag.display());
                let auth_state = app.state::<AuthState>();
                // Set memory state immediately — get_auth_status also checks file, but belt+suspenders
                *auth_state.0.lock().unwrap() = true;
                let auth_flag = Arc::clone(&auth_state.0);

                // Recreate hidden WebView using existing cookies (no delay needed)
                std::thread::spawn(move || {
                    open_canvas_window(app_handle, auth_flag, true);
                });
            } else {
                eprintln!("[oculus] no auth flag — fresh session");
            }

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations(
                    "sqlite:oculus.db",
                    vec![tauri_plugin_sql::Migration {
                        version: 1,
                        description: "initial schema",
                        sql: r#"
CREATE TABLE IF NOT EXISTS subjects (
    id            INTEGER PRIMARY KEY,
    code          TEXT    NOT NULL,
    name          TEXT    NOT NULL,
    term_name     TEXT,
    is_current    INTEGER NOT NULL DEFAULT 0,
    workflow_state TEXT   NOT NULL DEFAULT 'available',
    selected      INTEGER NOT NULL DEFAULT 1,
    last_synced_at TEXT,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS sync_runs (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    finished_at      TEXT,
    status           TEXT    NOT NULL DEFAULT 'running',
    subjects_synced  INTEGER NOT NULL DEFAULT 0,
    pages_scraped    INTEGER NOT NULL DEFAULT 0,
    error            TEXT
);

CREATE TABLE IF NOT EXISTS sync_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id     INTEGER REFERENCES sync_runs(id) ON DELETE SET NULL,
    subject_id INTEGER REFERENCES subjects(id)  ON DELETE SET NULL,
    timestamp  TEXT    NOT NULL DEFAULT (datetime('now')),
    level      TEXT    NOT NULL DEFAULT 'info',
    message    TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS files (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_id    INTEGER NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    filename      TEXT    NOT NULL,
    relative_path TEXT    NOT NULL,
    file_type     TEXT    NOT NULL,
    size_bytes    INTEGER,
    scraped_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(subject_id, relative_path)
);

CREATE TABLE IF NOT EXISTS settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
                        "#,
                        kind: tauri_plugin_sql::MigrationKind::Up,
                    },
                    tauri_plugin_sql::Migration {
                        version: 2,
                        description: "file metadata: category, source_url, canvas_id, modified_at",
                        sql: r#"
ALTER TABLE files ADD COLUMN category    TEXT;
ALTER TABLE files ADD COLUMN source_url  TEXT;
ALTER TABLE files ADD COLUMN canvas_id   INTEGER;
ALTER TABLE files ADD COLUMN modified_at TEXT;
                        "#,
                        kind: tauri_plugin_sql::MigrationKind::Up,
                    }],
                )
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_auth_status,
            launch_canvas_auth,
            disconnect_canvas,
            sync_subjects,
            scrape_content,
            cancel_scrape,
            rescrape_file,
            read_course_file,
            open_course_file,
            get_subjects,
            open_canvas_devtools,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

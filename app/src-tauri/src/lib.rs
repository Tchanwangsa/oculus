use std::io::Read as _;
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

/// Shared window-creation logic used by both startup restore and user-initiated auth.
/// `silent` = true → start hidden, navigate to canvas home (relies on existing cookies).
/// `silent` = false → start visible, navigate to SAML login.
fn open_canvas_window(app: AppHandle, auth_flag: Arc<Mutex<bool>>, silent: bool) {
    // Close any existing window first
    if let Some(existing) = app.get_webview_window("canvas-auth") {
        existing.close().ok();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let url = if silent {
        "https://canvas.lms.unimelb.edu.au/"
    } else {
        "https://canvas.lms.unimelb.edu.au/login/saml"
    };

    let session_dir   = canvas_session_dir(&app);
    let auth_flag_win = Arc::clone(&auth_flag);
    let app_nav       = app.clone();
    let app_win       = app.clone();
    let flag_path     = auth_flag_path(&app);

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
        let host_ok = url.host_str() == Some("canvas.lms.unimelb.edu.au");
        let path    = url.path();

        let authenticated = host_ok
            && (path == "/"
                || path.starts_with("/dashboard")
                || path.starts_with("/courses")
                || path.starts_with("/calendar")
                || path.starts_with("/inbox"));

        let on_login = host_ok && path.starts_with("/login");

        if authenticated {
            let already_done = {
                let mut flag = auth_flag.lock().unwrap();
                let prev = *flag;
                *flag = true;
                prev
            };
            if !already_done {
                // Persist auth so next app launch skips login
                std::fs::create_dir_all(flag_path.parent().unwrap()).ok();
                std::fs::write(&flag_path, b"1").ok();

                if let Some(w) = app_nav.get_webview_window("canvas-auth") {
                    w.hide().ok();
                }
                app_nav.emit("canvas-auth-success", "ok").ok();
            }
        } else if silent && on_login {
            // Session expired — close silently, reset state
            eprintln!("[oculus] silent auth: session expired, redirected to login");
            *auth_flag.lock().unwrap() = false;
            if let Some(w) = app_nav.get_webview_window("canvas-auth") {
                w.close().ok();
            }
            app_nav.emit("canvas-auth-expired", "expired").ok();
        }

        true
    })
    .build();

    match result {
        Ok(win) => {
            // Only emit cancelled for user-initiated auth (not silent restore)
            if !silent {
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        let authenticated = *auth_flag_win.lock().unwrap();
                        if !authenticated {
                            app_win.emit("canvas-auth-cancelled", "cancelled").ok();
                        }
                    }
                });
            }
        }
        Err(e) => eprintln!("[oculus] failed to open canvas window: {e}"),
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
            // Window gone (startup race or crash) — reset auth state and notify UI
            *auth.0.lock().unwrap() = false;
            app.emit("canvas-auth-expired", "window-missing").ok();

            // If session cookies still exist, recreate the window in background
            // so the user only needs to click "Connect" once more (not re-login)
            if auth_flag_path(&app).exists() {
                let app2 = app.clone();
                let flag  = Arc::clone(&auth.0);
                std::thread::spawn(move || open_canvas_window(app2, flag, true));
            }

            return Err("Canvas session window not ready — reconnecting, please try again in a moment.".to_string());
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
                        let mut body = String::new();
                        let _ = request.as_reader().read_to_string(&mut body);
                        eprintln!("[oculus] IPC POST {url} len={}", body.len());

                        if url.starts_with("/subjects") {
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
                        } else if url.starts_with("/error") {
                            eprintln!("[oculus] canvas error: {body}");
                            handle.emit("subjects-error", body).ok();
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
                    }],
                )
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_auth_status,
            launch_canvas_auth,
            disconnect_canvas,
            sync_subjects,
            get_subjects,
            open_canvas_devtools,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

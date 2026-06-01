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

fn cors_header(key: &[u8], val: &[u8]) -> tiny_http::Header {
    tiny_http::Header::from_bytes(key, val).unwrap()
}

fn cors_response(status: u16) -> tiny_http::Response<std::io::Empty> {
    tiny_http::Response::empty(status)
        .with_header(cors_header(b"Access-Control-Allow-Origin", b"*"))
        .with_header(cors_header(b"Access-Control-Allow-Methods", b"POST, OPTIONS"))
        .with_header(cors_header(b"Access-Control-Allow-Headers", b"Content-Type"))
}

#[tauri::command]
fn get_auth_status(state: tauri::State<AuthState>) -> bool {
    *state.0.lock().unwrap()
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
async fn sync_subjects(
    app: AppHandle,
    port: tauri::State<'_, IpcPort>,
) -> Result<(), String> {
    let win = app
        .get_webview_window("canvas-auth")
        .ok_or_else(|| "Not authenticated — connect to Canvas first".to_string())?;

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
        console.log('[Oculus] fetching Canvas courses API...');
        const resp = await fetch(
            '/api/v1/courses?per_page=100&include[]=term&include[]=account',
            {{ credentials: 'include' }}
        );
        console.log('[Oculus] API status:', resp.status);
        if (!resp.ok) throw new Error(`Canvas API ${{resp.status}}`);

        // 1. Scrape DOM tables — Canvas already separates current vs past for us
        const scrapeTable = (tableId, isCurrent) => {{
            const table = document.querySelector(tableId);
            if (!table) {{ console.warn('[Oculus] table not found:', tableId); return []; }}
            return [...table.querySelectorAll('tr[id^="course_"]')].map(row => {{
                const link = row.querySelector('td a');
                const idMatch = link?.href?.match(/\/courses\/(\d+)/);
                return idMatch ? {{ id: parseInt(idMatch[1]), _oculus_is_current: isCurrent }} : null;
            }}).filter(Boolean);
        }};

        const currentDom = scrapeTable('#my_courses_table', true);
        const pastDom    = scrapeTable('#past_enrollments_table', false);
        console.log('[Oculus] DOM current:', currentDom.length, 'past:', pastDom.length);

        if (currentDom.length === 0 && pastDom.length === 0) {{
            throw new Error('DOM tables not found — ensure Canvas dashboard is loaded. Re-authenticate and try again.');
        }}

        const domMap = new Map([...currentDom, ...pastDom].map(c => [c.id, c]));

        // 2. Fetch API for full metadata (name, course_code, term details)
        const all = await resp.json();
        console.log('[Oculus] API courses:', all.length);

        // 3. Merge: only keep courses visible in DOM tables (auto-filters communities etc.)
        const courses = all
            .filter(c => domMap.has(c.id))
            .map(c => ({{ ...c, _oculus_is_current: domMap.get(c.id)._oculus_is_current }}));

        console.log('[Oculus] merged courses:', courses.length,
            '| current:', courses.filter(c => c._oculus_is_current).length,
            '| past:', courses.filter(c => !c._oculus_is_current).length);

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

#[tauri::command]
async fn launch_canvas_auth(
    app: AppHandle,
    state: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("canvas-auth") {
        existing.close().map_err(|e| e.to_string())?;
    }

    let auth_flag     = Arc::clone(&state.0);
    let auth_flag_win = Arc::clone(&state.0);
    let app_nav       = app.clone();
    let app_win       = app.clone();
    let session_dir   = canvas_session_dir(&app);

    let win = WebviewWindowBuilder::new(
        &app,
        "canvas-auth",
        WebviewUrl::External(
            "https://canvas.lms.unimelb.edu.au/login/saml"
                .parse()
                .unwrap(),
        ),
    )
    .title("Sign in to Canvas — Oculus")
    .inner_size(900.0, 700.0)
    .center()
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

        if authenticated {
            let already_done = {
                let mut flag = auth_flag.lock().unwrap();
                let prev = *flag;
                *flag = true;
                prev
            };
            if !already_done {
                if let Some(w) = app_nav.get_webview_window("canvas-auth") {
                    w.hide().ok();
                }
                app_nav.emit("canvas-auth-success", "ok").ok();
            }
        }
        true
    })
    .build()
    .map_err(|e| e.to_string())?;

    win.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { .. } = event {
            let authenticated = *auth_flag_win.lock().unwrap();
            if !authenticated {
                app_win.emit("canvas-auth-cancelled", "cancelled").ok();
            }
        }
    });

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

    let session_dir = canvas_session_dir(&app);
    if session_dir.exists() {
        std::fs::remove_dir_all(&session_dir).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AuthState(Arc::new(Mutex::new(false))))
        .manage(SubjectsState(Arc::new(Mutex::new(vec![]))))
        .setup(|app| {
            // Bind to port 0 → OS picks an available port
            let server = tiny_http::Server::http("127.0.0.1:0")
                .expect("[oculus] failed to start IPC HTTP server");
            let port = server
                .server_addr()
                .to_ip()
                .expect("IPC server addr missing")
                .port();

            eprintln!("[oculus] IPC HTTP server listening on 127.0.0.1:{port}");
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

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
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

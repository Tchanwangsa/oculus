use std::io::Read;
use tauri::{AppHandle, Emitter, Manager};

use crate::cors::{cors_header, cors_response, with_cors};
use crate::files::{category_from_path, parse_query, proxy_cookie, write_course_bytes};
use crate::subjects::SubjectsState;

pub struct IpcPort(pub u16);

// ── Server entry point ────────────────────────────────────────────────────────

pub fn start_ipc_server(app: AppHandle) -> u16 {
    let server =
        tiny_http::Server::http("127.0.0.1:0").expect("[oculus] failed to start IPC HTTP server");
    let port = server
        .server_addr()
        .to_ip()
        .expect("IPC server addr missing")
        .port();

    eprintln!("[oculus] IPC HTTP server on 127.0.0.1:{port}");

    let handle = app.clone();
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let method = request.method().clone();
            let url = request.url().to_string();

            if method == tiny_http::Method::Options {
                let _ = request.respond(cors_response(200));
                continue;
            }

            if method == tiny_http::Method::Get {
                handle_get(&url, &handle, request);
                continue;
            }

            if method == tiny_http::Method::Post {
                let mut bytes: Vec<u8> = Vec::new();
                let _ = request.as_reader().read_to_end(&mut bytes);
                eprintln!("[oculus] IPC POST {url} len={}", bytes.len());

                route_request(&url, &bytes, &handle);

                let _ = request.respond(cors_response(200));
            }
        }
    });

    port
}

// ── Route dispatcher ──────────────────────────────────────────────────────────

fn route_request(url: &str, bytes: &[u8], handle: &AppHandle) {
    // Order matters: more-specific prefixes must come before the generic /scrape
    // since /scrape-progress, /scrape-binary etc. all start with /scrape.
    if url.starts_with("/subjects")              { handle_subjects(bytes, handle); }
    else if url.starts_with("/image-proxy")      { handle_image_proxy(url, bytes, handle); }
    else if url.starts_with("/parse-status")     { handle_emit_json(bytes, handle, "parse-status"); }
    else if url.starts_with("/scrape-progress")  { handle_emit_json(bytes, handle, "scrape-progress"); }
    else if url.starts_with("/scrape-done")      { handle_emit_json(bytes, handle, "scrape-complete"); }
    else if url.starts_with("/scrape-log")       { handle_emit_json(bytes, handle, "scrape-log"); }
    else if url.starts_with("/scrape-binary")    { handle_scrape_artifact(url, bytes, handle, true); }
    else if url.starts_with("/scrape")           { handle_scrape_artifact(url, bytes, handle, false); }
    else if url.starts_with("/error")            { handle_error(bytes, handle); }
}

// ── GET routes (worker page + Canvas cookie proxy) ──────────────────────────

fn handle_get(url: &str, handle: &AppHandle, request: tiny_http::Request) {
    if url.starts_with("/worker") {
        // Blank host page for the hidden worker WebView. Same-origin as this
        // server, so the scraper's fetches to /canvas and /scrape need no CORS.
        let html = "<!doctype html><html><head><meta charset=\"utf-8\">\
                    <title>oculus worker</title></head><body></body></html>";
        let resp = with_cors(
            tiny_http::Response::from_string(html)
                .with_header(cors_header(b"Content-Type", b"text/html; charset=utf-8")),
        );
        let _ = request.respond(resp);
    } else if url.starts_with("/canvas") {
        handle_canvas_proxy(url, handle, request);
    } else {
        let _ = request.respond(cors_response(404));
    }
}

/// Reverse-proxy a Canvas request, attaching the persisted session cookie.
/// `GET /canvas?url=<absolute canvas url>` → forwards status, body, and the
/// `Link` header (for the scraper's pagination). This is what lets data
/// fetching survive a restart: the WebView is logged out, but Rust still has
/// the cookie.
fn handle_canvas_proxy(url: &str, handle: &AppHandle, request: tiny_http::Request) {
    let qp = parse_query(url);
    let target = match qp.get("url") {
        Some(u) if !u.is_empty() => u.clone(),
        _ => {
            let _ = request.respond(cors_response(400));
            return;
        }
    };
    // Only ever proxy to Canvas — never an arbitrary URL.
    if !target.starts_with("https://canvas.lms.unimelb.edu.au/") {
        eprintln!("[oculus] canvas proxy: refused non-Canvas url {target}");
        let _ = request.respond(cors_response(403));
        return;
    }

    let cookie = proxy_cookie(handle);
    let mut req = ureq::get(&target);
    if !cookie.is_empty() {
        req = req.set("Cookie", &cookie);
    }

    match req.call() {
        Ok(resp) => respond_proxy(request, resp),
        // Canvas 4xx/5xx still carry a body — forward it so the scraper can
        // branch on r.status (e.g. skip 403/404).
        Err(ureq::Error::Status(_, resp)) => respond_proxy(request, resp),
        Err(e) => {
            eprintln!("[oculus] canvas proxy fetch failed: {e}");
            let _ = request.respond(cors_response(502));
        }
    }
}

fn respond_proxy(request: tiny_http::Request, resp: ureq::Response) {
    let status = resp.status();
    let ct = resp
        .header("content-type")
        .unwrap_or("application/octet-stream")
        .to_string();
    let link = resp.header("Link").map(|s| s.to_string());

    let mut body: Vec<u8> = Vec::new();
    let _ = resp.into_reader().read_to_end(&mut body);

    let mut out = with_cors(tiny_http::Response::from_data(body).with_status_code(status))
        .with_header(cors_header(b"Content-Type", ct.as_bytes()));
    if let Some(l) = link {
        out = out.with_header(cors_header(b"Link", l.as_bytes()));
    }
    let _ = request.respond(out);
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// Scraper JS sends the full course list. Parse, store in state, and tell the
/// frontend so it can upsert them into the database.
fn handle_subjects(bytes: &[u8], handle: &AppHandle) {
    let body = String::from_utf8_lossy(bytes);
    match serde_json::from_str::<Vec<serde_json::Value>>(&body) {
        Ok(courses) => {
            eprintln!("[oculus] parsed {} courses", courses.len());
            *handle.state::<SubjectsState>().0.lock().unwrap() = courses.clone();
            handle.emit("subjects-loaded", courses).ok();
        }
        Err(e) => {
            eprintln!("[oculus] parse error: {e}");
            handle.emit("subjects-error", e.to_string()).ok();
        }
    }
}

/// JS can't download images directly (CORS on CDN). It POSTs the image URL here
/// and Rust fetches it server-side, writes it to disk, emits a scrape-file event.
fn handle_image_proxy(url: &str, bytes: &[u8], handle: &AppHandle) {
    let qp   = parse_query(url);
    let code = qp.get("course").map(String::as_str).unwrap_or("UNKNOWN");
    let sid  = qp.get("subject_id").and_then(|s| s.parse().ok()).unwrap_or(0);
    let path = qp.get("path").map(String::as_str).unwrap_or("");
    let cid  = qp.get("canvas_id").and_then(|s| s.parse().ok());
    let cdn  = String::from_utf8_lossy(bytes).trim().to_string();

    if path.is_empty() || cdn.is_empty() {
        handle.emit("scrape-error", "image-proxy: missing path or url").ok();
        return;
    }

    let cookie = proxy_cookie(handle);
    let mut req = ureq::get(&cdn);
    if !cookie.is_empty() {
        req = req.set("Cookie", &cookie);
    }

    match req.call() {
        Ok(resp) => {
            let ct = resp.content_type().to_string();
            let mut img: Vec<u8> = Vec::new();
            let _ = resp.into_reader().read_to_end(&mut img);

            if ct.contains("text/html") {
                // Auth cookie expired — CDN returned a login page instead of image
                eprintln!("[oculus] image-proxy: HTML response (auth failed) for {path}");
                handle.emit("scrape-log", serde_json::json!({
                    "level": "warning", "course": code,
                    "message": format!("image {path}: got HTML (not image) — auth/url issue")
                })).ok();
            } else {
                eprintln!("[oculus] image-proxy: {} bytes ({ct}) for {}", img.len(), path);
                emit_file_event(handle, code, sid, path, &img, cid);
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

/// Writes text or binary content to disk and emits scrape-file to the frontend.
/// `is_binary` = true → body IS the raw file bytes.
/// `is_binary` = false → the file path/metadata are in query params, body is text.
fn handle_scrape_artifact(url: &str, bytes: &[u8], handle: &AppHandle, is_binary: bool) {
    let qp   = parse_query(url);
    let code = qp.get("course").map(String::as_str).unwrap_or("UNKNOWN");
    let sid  = qp.get("subject_id").and_then(|s| s.parse().ok()).unwrap_or(0);
    let path = qp.get("path").map(String::as_str).unwrap_or("");
    let cid  = qp.get("canvas_id").and_then(|s| s.parse().ok());

    if path.is_empty() {
        let tag = if is_binary { "binary" } else { "scrape" };
        handle.emit("scrape-error", format!("{tag}: missing path")).ok();
        return;
    }

    emit_file_event(handle, code, sid, path, bytes, cid);
}

/// JSON-only route: parse the body as JSON and emit it raw to the frontend.
fn handle_emit_json(bytes: &[u8], handle: &AppHandle, event: &str) {
    let body = String::from_utf8_lossy(bytes);
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
        handle.emit(event, v).ok();
    }
}

/// JS scraper hit an unexpected error. Pass the message through to the frontend.
fn handle_error(bytes: &[u8], handle: &AppHandle) {
    let body = String::from_utf8_lossy(bytes);
    eprintln!("[oculus] canvas error: {body}");
    handle.emit("subjects-error", body.to_string()).ok();
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Write bytes to disk under `courses/{code}/{path}`, then notify the frontend
/// so it can record the file in the database.
fn emit_file_event(
    handle: &AppHandle,
    code: &str,
    subject_id: i64,
    path: &str,
    data: &[u8],
    canvas_id: Option<i64>,
) {
    match write_course_bytes(handle, code, path, data) {
        Ok((rel, n)) => {
            eprintln!("[oculus] wrote {rel} ({n} bytes)");
            handle
                .emit(
                    "scrape-file",
                    serde_json::json!({
                        "subject_id": subject_id,
                        "code": code,
                        "relative_path": rel,
                        "size_bytes": n,
                        "category": category_from_path(path),
                        "canvas_id": canvas_id,
                    }),
                )
                .ok();

            if path.ends_with(".pdf") {
                if let Ok(abs) = handle.path().app_data_dir() {
                    let abs_path = abs.join(&rel).to_string_lossy().to_string();
                    let code_owned = code.to_string();
                    let rel_owned = rel.clone();
                    let ipc_port = handle.state::<IpcPort>().0;
                    let sid = subject_id;
                    std::thread::spawn(move || {
                        trigger_pdf_parse(&abs_path, &code_owned, &rel_owned, sid, ipc_port);
                    });
                }
            }
        }
        Err(e) => {
            eprintln!("[oculus] write failed: {e}");
            handle.emit("scrape-error", e).ok();
        }
    }
}

fn trigger_pdf_parse(abs_path: &str, code: &str, rel_path: &str, subject_id: i64, ipc_port: u16) {
    let body = serde_json::json!({
        "pdf_path": abs_path,
        "subject_code": code,
        "relative_path": rel_path,
        "subject_id": subject_id,
        "ipc_port": ipc_port,
    });
    match ureq::post("http://127.0.0.1:9547/parse-pdf")
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
    {
        Ok(r) => eprintln!("[oculus] parse-pdf {}: {}", abs_path, r.status()),
        Err(e) => eprintln!("[oculus] parse-pdf sidecar not available: {e}"),
    }
}

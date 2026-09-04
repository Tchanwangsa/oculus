//! Callback server for the Python sidecar.
//!
//! This used to be a much larger surface: a Canvas cookie proxy, artifact
//! upload routes and a blank page to host the scraper WebView, all so JS
//! running in a hidden window could reach Canvas and hand results back. The
//! scraper is Rust now (see `sync.rs`), so the only thing left that needs to
//! call in from another process is the sidecar telling us how a PDF parse is
//! going.

use tauri::{AppHandle, Emitter};

pub struct IpcPort(pub u16);

/// The sidecar posts a handful of status updates per PDF; two workers is
/// ample, and keeps one slow emit from blocking the next update.
const IPC_WORKERS: usize = 2;

pub fn start_ipc_server(app: AppHandle) -> u16 {
    let server =
        tiny_http::Server::http("127.0.0.1:0").expect("[oculus] failed to start IPC HTTP server");
    let port = server
        .server_addr()
        .to_ip()
        .expect("IPC server addr missing")
        .port();

    eprintln!("[oculus] IPC HTTP server on 127.0.0.1:{port}");

    let server = std::sync::Arc::new(server);
    for _ in 0..IPC_WORKERS {
        let server = std::sync::Arc::clone(&server);
        let handle = app.clone();
        std::thread::spawn(move || {
            while let Ok(mut request) = server.recv() {
                let is_parse_status =
                    request.method() == &tiny_http::Method::Post && request.url().starts_with("/parse-status");

                if is_parse_status {
                    let mut bytes: Vec<u8> = Vec::new();
                    let _ = request.as_reader().read_to_end(&mut bytes);
                    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        handle.emit("parse-status", v).ok();
                    }
                    let _ = request.respond(tiny_http::Response::empty(200));
                } else {
                    let _ = request.respond(tiny_http::Response::empty(404));
                }
            }
        });
    }

    port
}

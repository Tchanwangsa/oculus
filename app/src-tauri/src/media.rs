//! Localhost HTTP server for media playback.
//!
//! WebKit's media pipeline refuses to load `<video>`/`<audio>` sources from
//! custom URL schemes — `fetch()` of an `asset://` URL returns the bytes
//! fine, but the media element fails instantly with
//! MEDIA_ERR_SRC_NOT_SUPPORTED before making a single request (observed on
//! macOS 26; the same class of failure is tauri-apps/tauri#3725). Real HTTP
//! is the only origin the media stack accepts for local files, so lecture
//! video is served from this tiny server instead of the asset protocol.
//!
//! Scope: only files under the data dir's `lectures/` and `courses/` trees
//! (the same scope as the asset protocol), and only with the per-launch
//! token in the URL path — the port is reachable by any local process, so
//! requests without the token are rejected outright.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;

use tiny_http::{Header, Method, Response, Server};

/// Managed state: where the media server is listening this launch.
pub struct MediaServer {
    pub port: u16,
    pub token: String,
}

/// Concurrent range requests happen on every seek; a few workers keep one
/// slow disk read from stalling playback.
const MEDIA_WORKERS: usize = 4;

#[derive(serde::Serialize)]
pub struct MediaServerInfo {
    port: u16,
    token: String,
}

#[tauri::command]
pub fn media_server_info(state: tauri::State<MediaServer>) -> MediaServerInfo {
    MediaServerInfo {
        port: state.port,
        token: state.token.clone(),
    }
}

pub fn start_media_server(data_dir: PathBuf) -> MediaServer {
    let server = Server::http("127.0.0.1:0").expect("[oculus] failed to start media HTTP server");
    let port = server
        .server_addr()
        .to_ip()
        .expect("media server addr missing")
        .port();
    let token = random_token();

    eprintln!("[oculus] media HTTP server on 127.0.0.1:{port}");

    let server = Arc::new(server);
    for _ in 0..MEDIA_WORKERS {
        let server = Arc::clone(&server);
        let data_dir = data_dir.clone();
        let token = token.clone();
        std::thread::spawn(move || {
            while let Ok(request) = server.recv() {
                handle(request, &data_dir, &token);
            }
        });
    }

    MediaServer { port, token }
}

fn handle(request: tiny_http::Request, data_dir: &PathBuf, token: &str) {
    let respond_status = |request: tiny_http::Request, code: u16| {
        let _ = request.respond(Response::empty(code));
    };

    if request.method() != &Method::Get && request.method() != &Method::Head {
        return respond_status(request, 405);
    }

    // URL shape: /{token}?path=<absolute path>. Url::parse handles the
    // percent-decoding of the query pair.
    let Ok(url) = url::Url::parse(&format!("http://localhost{}", request.url())) else {
        return respond_status(request, 400);
    };
    if url.path().trim_matches('/') != token {
        return respond_status(request, 403);
    }
    let Some(path) = url
        .query_pairs()
        .find(|(k, _)| k == "path")
        .map(|(_, v)| PathBuf::from(v.as_ref()))
    else {
        return respond_status(request, 400);
    };

    // Canonicalize before the scope check so `..` segments can't escape.
    let Ok(path) = path.canonicalize() else {
        return respond_status(request, 404);
    };
    let in_scope = ["lectures", "courses"]
        .iter()
        .any(|dir| path.starts_with(data_dir.join(dir)));
    if !in_scope {
        return respond_status(request, 403);
    }

    let Ok(mut file) = File::open(&path) else {
        return respond_status(request, 404);
    };
    let Ok(total) = file.metadata().map(|m| m.len()) else {
        return respond_status(request, 500);
    };

    let content_type = match path.extension().and_then(|e| e.to_str()) {
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("m4a") => "audio/mp4",
        Some("mp3") => "audio/mpeg",
        Some("vtt") => "text/vtt",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    };
    let hdr = |k: &str, v: &str| Header::from_bytes(k.as_bytes(), v.as_bytes()).unwrap();

    let range = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Range"))
        .and_then(|h| parse_range(h.value.as_str(), total));

    let is_head = request.method() == &Method::Head;

    match range {
        Some((start, end)) => {
            let len = end - start + 1;
            if file.seek(SeekFrom::Start(start)).is_err() {
                return respond_status(request, 500);
            }
            let body: Box<dyn Read + Send> = if is_head {
                Box::new(std::io::empty())
            } else {
                Box::new(file.take(len))
            };
            let response = Response::new(206.into(), vec![], body, Some(len as usize), None)
                .with_header(hdr("Content-Type", content_type))
                .with_header(hdr("Accept-Ranges", "bytes"))
                .with_header(hdr(
                    "Content-Range",
                    &format!("bytes {start}-{end}/{total}"),
                ));
            let _ = request.respond(response);
        }
        None => {
            let body: Box<dyn Read + Send> = if is_head {
                Box::new(std::io::empty())
            } else {
                Box::new(file)
            };
            let response = Response::new(200.into(), vec![], body, Some(total as usize), None)
                .with_header(hdr("Content-Type", content_type))
                .with_header(hdr("Accept-Ranges", "bytes"));
            let _ = request.respond(response);
        }
    }
}

/// Parse a single-range `Range: bytes=a-b` header into inclusive (start, end).
/// Returns None for anything malformed or unsatisfiable — the caller then
/// serves the whole file, which every media client copes with.
fn parse_range(value: &str, total: u64) -> Option<(u64, u64)> {
    let spec = value.strip_prefix("bytes=")?.split(',').next()?.trim();
    let (start_s, end_s) = spec.split_once('-')?;
    if total == 0 {
        return None;
    }
    if start_s.is_empty() {
        // suffix form: last N bytes
        let n: u64 = end_s.parse().ok()?;
        if n == 0 {
            return None;
        }
        let start = total.saturating_sub(n);
        return Some((start, total - 1));
    }
    let start: u64 = start_s.parse().ok()?;
    if start >= total {
        return None;
    }
    let end = if end_s.is_empty() {
        total - 1
    } else {
        end_s.parse::<u64>().ok()?.min(total - 1)
    };
    (start <= end).then_some((start, end))
}

/// Per-launch bearer token. /dev/urandom is always present on macOS; the
/// fallback only exists so a broken read degrades to "still unguessable in
/// practice" instead of a panic.
fn random_token() -> String {
    let mut buf = [0u8; 16];
    if let Ok(mut f) = File::open("/dev/urandom") {
        if f.read_exact(&mut buf).is_ok() {
            return buf.iter().map(|b| format!("{b:02x}")).collect();
        }
    }
    format!(
        "{:x}{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    )
}

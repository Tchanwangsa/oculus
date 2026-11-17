//! Localhost HTTP server for media playback. WebKit's media element rejects
//! custom-scheme sources (`asset://` fails with MEDIA_ERR_SRC_NOT_SUPPORTED on
//! macOS 26; cf. tauri-apps/tauri#3725), so lecture video is served over HTTP.
//!
//! Scope: files under the library's roots (`layout::library_roots`, which
//! hold the lecture recordings) and `studio/videos/` (rendered videos and
//! their stills), and only with the
//! per-launch token in the path — any local process can reach the port.
//!
//! One route needs no token: `/widget-kit/**`, the code and fonts chat
//! widgets run on (`widgets/kit.rs`), public files with CORS `*`.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::widgets::kit::{self, Kit};
use tiny_http::{Header, Method, Response, Server};

/// Managed state: where the media server is listening this launch.
pub struct MediaServer {
    pub port: u16,
    pub token: String,
}

/// A few workers, so one slow range read does not stall a seek.
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

pub fn start_media_server(data_dir: PathBuf, kit: Kit) -> MediaServer {
    let server = Server::http("127.0.0.1:0").expect("[oculus] failed to start media HTTP server");
    let port = server
        .server_addr()
        .to_ip()
        .expect("media server addr missing")
        .port();
    let token = random_token();

    elog!("[oculus] media HTTP server on 127.0.0.1:{port}");

    let server = Arc::new(server);
    let kit = Arc::new(kit);
    for _ in 0..MEDIA_WORKERS {
        let server = Arc::clone(&server);
        let data_dir = data_dir.clone();
        let token = token.clone();
        let kit = Arc::clone(&kit);
        std::thread::spawn(move || {
            while let Ok(request) = server.recv() {
                handle(request, &data_dir, &token, &kit);
            }
        });
    }

    MediaServer { port, token }
}

fn handle(request: tiny_http::Request, data_dir: &Path, token: &str, kit: &Kit) {
    let respond_status = |request: tiny_http::Request, code: u16| {
        let _ = request.respond(Response::empty(code));
    };

    if request.method() != &Method::Get && request.method() != &Method::Head {
        return respond_status(request, 405);
    }

    // URL shape: /{token}?path=<absolute path>.
    let Ok(url) = url::Url::parse(&format!("http://localhost{}", request.url())) else {
        return respond_status(request, 400);
    };
    if let Some(rest) = url.path().strip_prefix(kit::ROUTE) {
        return respond_kit(request, kit, rest);
    }
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
    let Ok(path) = crate::runtime::platform::canonicalize(path) else {
        return respond_status(request, 404);
    };
    let in_scope = crate::library::layout::library_roots()
        .into_iter()
        .map(|root| data_dir.join(root))
        .chain([crate::library::paths::studio_dir(data_dir).join("videos")])
        .any(|dir| {
            crate::runtime::platform::canonicalize(dir).is_ok_and(|scope| path.starts_with(scope))
        });
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
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("m4a") => "audio/mp4",
        Some("mp3") => "audio/mpeg",
        Some("vtt") => "text/vtt",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
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

/// A kit file (or its 404) with the kit's CORS and caching headers.
fn respond_kit(request: tiny_http::Request, kit: &Kit, rest: &str) {
    let reply = kit.respond(request.method().as_str(), rest);
    let hdr = |k: &str, v: &str| Header::from_bytes(k.as_bytes(), v.as_bytes()).unwrap();
    let len = reply.body.len();
    let body: Box<dyn Read + Send> = if request.method() == &Method::Head {
        Box::new(std::io::empty())
    } else {
        Box::new(std::io::Cursor::new(reply.body))
    };
    let mut response = Response::new(reply.status.into(), vec![], body, Some(len), None);
    for (k, v) in kit::HEADERS {
        response = response.with_header(hdr(k, v));
    }
    if let Some(content_type) = reply.content_type {
        response = response.with_header(hdr("Content-Type", content_type));
    }
    let _ = request.respond(response);
}

/// Parse `Range: bytes=a-b` into inclusive (start, end); `None` for anything
/// malformed or unsatisfiable, and the caller serves the whole file.
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

/// The loopback server requires an unpredictable per-launch bearer token.
fn random_token() -> String {
    use ring::rand::{SecureRandom, SystemRandom};
    let mut buf = [0u8; 16];
    SystemRandom::new()
        .fill(&mut buf)
        .expect("media bearer token entropy unavailable");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;
    use std::io::Write;

    fn get(port: u16, path: &str) -> String {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        out.to_ascii_lowercase()
    }

    #[test]
    fn kit_route_needs_no_token_and_only_it_has_cors() {
        let scratch = Scratch::new("media-kit");
        let root = scratch.join("kit");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("core.js"), "export {}").unwrap();
        let server = start_media_server(scratch.join("data"), Kit::disk(root));

        let kit = get(server.port, "/widget-kit/core.js?x=1");
        assert!(kit.starts_with("http/1.1 200"), "{kit}");
        assert!(kit.contains("access-control-allow-origin: *"), "{kit}");
        assert!(kit.contains("x-content-type-options: nosniff"), "{kit}");
        assert!(kit.contains("content-type: text/javascript"), "{kit}");

        let missing = get(server.port, "/widget-kit/a%5Cb.js");
        assert!(missing.starts_with("http/1.1 404"), "{missing}");

        let video = get(server.port, "/not-the-token?path=/etc/hosts");
        assert!(video.starts_with("http/1.1 403"), "{video}");
        assert!(!video.contains("access-control-allow-origin"), "{video}");
    }
}

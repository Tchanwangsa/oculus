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
        .filter_map(|dir| data_dir.join(dir).canonicalize().ok())
        .any(|root| path.starts_with(root));
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

/// Per-launch bearer token from the platform's cryptographic random source.
fn random_token() -> String {
    let mut buf = [0u8; 16];
    getrandom::fill(&mut buf).expect("OS random source unavailable");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_unicode_paths_and_ranges_but_rejects_outside_files() {
        let dir = std::env::temp_dir().join(format!("oculus media 雪 {}", random_token()));
        std::fs::create_dir_all(dir.join("lectures")).unwrap();
        let video = dir.join("lectures").join("sample ü.mp4");
        let outside = dir.join("private.txt");
        std::fs::write(&video, b"0123456789").unwrap();
        std::fs::write(&outside, b"private").unwrap();
        let server = start_media_server(dir.clone());
        let make_url = |path: &std::path::Path, token: &str| {
            let mut url = url::Url::parse(&format!("http://127.0.0.1:{}/{}", server.port, token)).unwrap();
            url.query_pairs_mut().append_pair("path", &path.to_string_lossy());
            url.to_string()
        };
        let response = ureq::get(&make_url(&video, &server.token))
            .set("Range", "bytes=2-5").call().unwrap();
        assert_eq!(response.status(), 206);
        assert_eq!(response.header("Content-Range"), Some("bytes 2-5/10"));
        assert_eq!(response.into_string().unwrap(), "2345");
        for url in [make_url(&outside, &server.token), make_url(&video, "wrong")] {
            assert!(matches!(ureq::get(&url).call(), Err(ureq::Error::Status(403, _))));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

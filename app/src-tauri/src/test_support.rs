//! Scaffolding the unit tests share: scratch directories, small real PDFs, and
//! a fake HTTP server on loopback.

use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::ratelimit::hold;

// ── Scratch directories ──────────────────────────────────────────────────────

/// A fresh directory under the system temp dir, removed on drop. Derefs to
/// its path.
pub struct Scratch {
    root: PathBuf,
}

impl Scratch {
    pub fn new(name: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "oculus-{name}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }
}

impl Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.root
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.root
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

// ── PDFs ─────────────────────────────────────────────────────────────────────

/// A real PDF of `pages` blank 144pt-square pages.
pub fn write_pdf(path: &Path, pages: usize) {
    write_pdf_sized(path, pages, 144, 144);
}

pub fn write_pdf_sized(path: &Path, pages: usize, width: i64, height: i64) {
    use lopdf::{dictionary, Document, Object};
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let kids: Vec<Object> = (0..pages)
        .map(|_| {
            document
                .add_object(dictionary! {
                    "Type" => "Page",
                    "Parent" => pages_id,
                    "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
                })
                .into()
        })
        .collect();
    let count = kids.len() as i64;
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    document.trailer.set("Root", catalog);
    document.save(path).unwrap();
}

// ── A fake HTTP server ───────────────────────────────────────────────────────

/// One request the fake received.
#[derive(Clone)]
pub struct Hit {
    pub method: String,
    pub url: String,
    /// Field names lowercased.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// 0 for the first request this server saw.
    pub index: usize,
    /// `http://127.0.0.1:{port}` of this server, for replies that point back.
    pub origin: String,
}

impl Hit {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
}

pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
}

impl Reply {
    pub fn json(value: Value) -> Self {
        Self::status(200, value)
    }

    pub fn status(status: u16, value: Value) -> Self {
        (status, value.to_string().into_bytes()).into()
    }

    pub fn bytes(body: Vec<u8>) -> Self {
        (200, body).into()
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

impl From<(u16, Vec<u8>)> for Reply {
    fn from((status, body): (u16, Vec<u8>)) -> Self {
        Self {
            status,
            body,
            headers: Vec::new(),
        }
    }
}

/// A `tiny_http` server on a free loopback port, answering every request with
/// `handler` and logging it. Stopped on drop.
pub struct FakeServer {
    origin: String,
    hits: Arc<Mutex<Vec<Hit>>>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    pub fn start<H, R>(handler: H) -> Self
    where
        H: Fn(&Hit) -> R + Send + 'static,
        R: Into<Reply>,
    {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let origin = format!(
            "http://127.0.0.1:{}",
            server.server_addr().to_ip().unwrap().port()
        );
        let hits: Arc<Mutex<Vec<Hit>>> = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = {
            let (hits, stop, origin) = (hits.clone(), stop.clone(), origin.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    let Ok(Some(mut request)) = server.recv_timeout(Duration::from_millis(20))
                    else {
                        continue;
                    };
                    let mut body = Vec::new();
                    request.as_reader().read_to_end(&mut body).ok();
                    let hit = {
                        let mut log = hold(&hits);
                        let hit = Hit {
                            method: request.method().as_str().to_string(),
                            url: request.url().to_string(),
                            headers: request
                                .headers()
                                .iter()
                                .map(|header| {
                                    (
                                        header.field.as_str().to_string().to_lowercase(),
                                        header.value.as_str().to_string(),
                                    )
                                })
                                .collect(),
                            body,
                            index: log.len(),
                            origin: origin.clone(),
                        };
                        log.push(hit.clone());
                        hit
                    };
                    let reply: Reply = handler(&hit).into();
                    let mut response =
                        tiny_http::Response::from_data(reply.body).with_status_code(reply.status);
                    for (name, value) in reply.headers {
                        response.add_header(
                            tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes())
                                .unwrap(),
                        );
                    }
                    request.respond(response).ok();
                }
            })
        };
        Self {
            origin,
            hits,
            stop,
            handle: Some(handle),
        }
    }

    pub fn origin(&self) -> String {
        self.origin.clone()
    }

    pub fn hits(&self) -> Vec<Hit> {
        hold(&self.hits).clone()
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            handle.join().ok();
        }
    }
}

/// An origin nothing is listening on: bound to learn a free port, then closed.
pub fn dead_origin() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

// ── A fake oculus-keyd ───────────────────────────────────────────────────────

/// A stand-in `oculus-keyd` on `<dir>/keyd.sock`, speaking its framing: each
/// request's header and body go to `handler`, whose header and body are the
/// reply. Its thread ends with the test process.
pub struct FakeKeyd {
    requests: Arc<Mutex<Vec<(Value, Vec<u8>)>>>,
}

impl FakeKeyd {
    pub fn start<H>(dir: &Path, handler: H) -> Self
    where
        H: Fn(&Value, &[u8]) -> (Value, Vec<u8>) + Send + 'static,
    {
        use keyd_core::framing;

        let listener =
            keyd_core::platform::Listener::bind(&crate::paths::keyd_socket_path(dir)).unwrap();
        let requests: Arc<Mutex<Vec<(Value, Vec<u8>)>>> = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        std::thread::spawn(move || {
            while let Ok(conn) = listener.accept() {
                let mut stream = std::io::BufReader::new(conn);
                while let Ok(Some(line)) = framing::read_line(&mut stream, framing::MAX_LINE) {
                    let (header, len) = framing::parse_header(&line).unwrap();
                    let body = framing::read_body(&mut stream, len).unwrap();
                    let (reply, out) = handler(&header, &body);
                    let streamed = header.get("stream") == Some(&Value::Bool(true));
                    hold(&log).push((header, body));
                    // A streamed reply is its header, then the body until the
                    // connection closes, as oculus-keyd sends it.
                    if streamed {
                        framing::write_frame(stream.get_mut(), &reply, b"").ok();
                        stream.get_mut().write_all(&out).ok();
                        break;
                    }
                    framing::write_frame(stream.get_mut(), &reply, &out).ok();
                }
            }
        });
        Self { requests }
    }

    /// Every request so far, in order: header, then body.
    pub fn requests(&self) -> Vec<(Value, Vec<u8>)> {
        hold(&self.requests).clone()
    }

    /// The ops requested so far, in order.
    pub fn ops(&self) -> Vec<String> {
        self.requests()
            .iter()
            .map(|(h, _)| h["op"].as_str().unwrap_or("").to_string())
            .collect()
    }
}

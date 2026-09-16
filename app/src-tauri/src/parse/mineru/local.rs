//! MinerU running on this machine, reached over loopback.
//!
//! This file is short, and the reason it is short is the whole design. The
//! cloud client is 1,800 lines because MinerU's *public* API has no per-file
//! endpoint: a batch is submitted as a list of names, one signed upload URL
//! comes back per name, each file is `PUT` to its own URL, and one endpoint is
//! polled until every task reports done. Batching, split-task progress, a
//! daily ledger, two token buckets and a keychain all exist to serve that
//! shape. MinerU's own server offers `POST /file_parse` instead — one
//! multipart request in, one result ZIP out — so none of that machinery has
//! anything to do here.
//!
//! What is *not* duplicated is the part that matters: `render` turns the
//! content list into page records, it does not know which side produced the
//! list, and `return_content_list=true` is what makes that true. The flags
//! below are set to match the cloud client's hardcoded parameters
//! (`client::run_batch`) under this endpoint's names, so the same PDF renders
//! the same markdown whichever backend a user picked. They are not settings.
//!
//! Three things are absent by construction rather than by omission: there is
//! no quota to exhaust, no token to be rejected, and no server text in any
//! error — the last one is the seam's rule and holds here too, even though the
//! server is on the same machine as the person reading the message.

use std::fs;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::parse::{
    parse_config, Health, ParseError, ParseOutput, Parser, Progress, PARSER_VERSION,
};

use super::client::{find_content_list, page_count, safe_extract, transport_detail};
use super::render;

/// The `backend` stamped into every record this client writes.
pub const BACKEND: &str = "mineru-local";

// MinerU 3.x's synchronous parse endpoint, and its health check. The health
// path is `/health`, not `/v1/health`: the latter belongs to MinerU 4's V1
// service, which replaced `/file_parse` with an upload/job/download cycle this
// client does not speak. `probe` asks for it anyway, and only so it can say
// which of the two is running.
const PARSE_PATH: &str = "/file_parse";
const HEALTH_PATH: &str = "/health";
const V1_HEALTH_PATH: &str = "/v1/health";

/// **A connect timeout and no read timeout.** A parse on this machine's CPU is
/// minutes of silence and that is not a hang — the same rule the cloud path
/// lives by, for the same reason: a deadline above the work could only abandon
/// a parse that was still progressing. Connecting, on the other hand, either
/// happens immediately or the server is not running, so it answers fast enough
/// that "unreachable" is a useful word.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// The health check is the exception: it is a status line, and a status line
/// that never resolves is worse than one that says it could not tell.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(3);

const COPY_CHUNK: usize = 1024 * 1024;

/// **One local parse at a time.**
///
/// `sync.rs` fires one detached thread per PDF and leaves the cost of that to
/// the backend. On the cloud it costs nothing: every thread meets
/// `Batcher::shared()` and a library's worth of PDFs leaves as a handful of
/// windowed batches. Nothing here plays that part, and without one a full sync
/// opens one multipart POST per PDF against a server that is *this machine* —
/// a hundred sockets held for minutes each (there is no read timeout, by
/// design) at a process loading models into the same RAM the app is using.
///
/// So the queue sits on this side of the socket instead of in the server's
/// accept backlog. One permit, because MinerU's own server works a document at
/// a time regardless: a second request in flight buys no throughput and pays
/// for the memory twice. Waiters are not FIFO — `Mutex` promises no such thing
/// — which is fine for a queue whose order nobody can observe.
///
/// Poisoning is stepped over deliberately. A panic on some other parse thread
/// must not be what makes this engine stop working for the rest of the session.
static PARSE_GATE: Mutex<()> = Mutex::new(());

/// The form fields, under this endpoint's names.
///
/// `backend`/`lang_list` here are `model_version`/`language` in the cloud API
/// — different spellings of the same two choices — and the pair is what pins
/// the markdown: `pipeline` is the model this app's page records were built
/// with, and `ch` is MinerU's own default, whose multilingual model handles
/// the English coursework in this library. Changing either is a re-parse of
/// everything, not a setting.
///
/// `return_content_list` is the load-bearing one: the flat `.md` in the same
/// ZIP has no page boundaries and drops `header` items, which on slides are
/// the titles.
const FIELDS: [(&str, &str); 7] = [
    ("backend", "pipeline"),
    ("lang_list", "ch"),
    ("formula_enable", "true"),
    ("table_enable", "true"),
    ("return_content_list", "true"),
    ("return_images", "true"),
    ("response_format_zip", "true"),
];

pub struct MinerULocal {
    base_url: String,
}

impl MinerULocal {
    /// The client the app uses: the API root from the settings row.
    ///
    /// No `Result`, unlike the cloud's `from_config`. There is nothing to be
    /// missing — a loopback address authenticates nothing — so a server that
    /// is not running is not a construction error, it is the `Offline` the
    /// first request returns.
    pub fn from_config() -> Self {
        Self::new(&parse_config().base_url)
    }

    pub fn new(base_url: &str) -> Self {
        Self { base_url: base_url.trim().trim_end_matches('/').to_string() }
    }

    /// One multipart POST, streamed.
    ///
    /// `ureq` 2 has no multipart support, so the envelope is written by hand —
    /// but the file itself is never read into memory: the head, the open file
    /// and the closing boundary are chained into one reader and the length is
    /// computed from the three sizes, which also keeps the request out of
    /// chunked encoding.
    fn post_file_parse(&self, pdf: &Path, destination: &Path) -> Result<(), ParseError> {
        let file = fs::File::open(pdf)
            .map_err(|e| ParseError::Io(format!("open {}: {e}", pdf.display())))?;
        let size = file
            .metadata()
            .map_err(|e| ParseError::Io(format!("stat {}: {e}", pdf.display())))?
            .len();
        let name = pdf.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

        // Everything above is this process's own bookkeeping; from here down a
        // server is doing work, so queue. Held to the end of the function: the
        // result ZIP is still being written off the same socket.
        let _permit = PARSE_GATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        let boundary = boundary();
        let (head, tail) = envelope(&boundary, &name);
        let length = head.len() as u64 + size + tail.len() as u64;
        // `.take(size)` is not belt-and-braces: `Content-Length` is a promise
        // made from `metadata()` above, and the file is a library PDF a sync
        // could be rewriting underneath this. Growing past the promise would
        // leave the overflow in the socket for whatever reuses the pooled
        // connection to read as its status line; the bound turns that into a
        // clean truncation the server rejects.
        let body = Cursor::new(head)
            .chain(BufReader::with_capacity(COPY_CHUNK, file).take(size))
            .chain(Cursor::new(tail));

        let agent = ureq::AgentBuilder::new().timeout_connect(CONNECT_TIMEOUT).build();
        let sent = agent
            .post(&format!("{}{PARSE_PATH}", self.base_url))
            .set("Content-Type", &format!("multipart/form-data; boundary={boundary}"))
            .set("Content-Length", &length.to_string())
            .send(body);

        let mut reader = match sent {
            Ok(response) if (200..300).contains(&response.status()) => response.into_reader(),
            // **A 3xx arrives here as `Ok`, not as `Error::Status`.** `ureq`
            // will not replay an unsized body, so a 307/308 — or any redirect
            // with no `Location` — is handed back intact. Without this arm its
            // body is written as the result ZIP, `safe_extract` fails, and the
            // file is condemned `Document`, which is *not retryable*: the
            // sweep never looks at it again because a reverse proxy was in the
            // way. The cloud client guards the same door (`client.rs`).
            Ok(response) => return Err(http_failure(response.status())),
            Err(ureq::Error::Status(status, _)) => return Err(http_failure(status)),
            Err(ureq::Error::Transport(transport)) => {
                return Err(ParseError::Offline(transport_detail(&transport)))
            }
        };

        let file = fs::File::create(destination)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", destination.display())))?;
        let mut out = BufWriter::with_capacity(COPY_CHUNK, file);
        std::io::copy(&mut reader, &mut out)
            .map_err(|e| ParseError::Io(format!("read the result: {e}")))?;
        // Bound and flushed by hand, because `BufWriter`'s own flush happens in
        // `Drop` and `Drop` has nowhere to put an error. A temp volume that
        // fills during the last chunk would otherwise return `Ok` over a
        // truncated archive, and a truncated archive is `Document` — permanent,
        // for a disk that was full for a moment.
        out.flush().map_err(|e| ParseError::Io(format!("write {}: {e}", destination.display())))
    }
}

impl Parser for MinerULocal {
    fn parse(
        &self,
        pdf: &Path,
        images_dir: &Path,
        images_rel: &str,
        on_progress: &dyn Fn(Progress),
    ) -> Result<ParseOutput, ParseError> {
        // Counted from the PDF, never from the content list's largest
        // `page_idx`: a document whose last page yielded nothing has no item
        // to be counted by, and the record is one entry per page of the *file*.
        let total = page_count(pdf)?;

        // **Progress is the honest gap.** `/file_parse` blocks until the whole
        // document is done and offers nothing to subscribe to in between, so
        // what can be reported is the page count and then the finish. The
        // Python's local tier filled the silence with an EMA over previous
        // parses — a bar that moves while nothing is known — and that is the
        // one behaviour from it deliberately not ported. A counter that sits
        // still is true; a bar that lies is not.
        on_progress(Progress { pages_done: 0, total_pages: total, backend: BACKEND });

        let scratch = Scratch::new()?;
        let archive = scratch.root.join("result.zip");
        let extracted = scratch.root.join("result");
        self.post_file_parse(pdf, &archive)?;
        safe_extract(&archive, &extracted)?;

        // Globbed rather than named, as on the cloud path: the archive's
        // layout has changed between MinerU versions.
        let content_path = find_content_list(&extracted)
            .ok_or(ParseError::Document { code: "no-content-list".into() })?;
        let content: Value = fs::read_to_string(&content_path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .ok_or(ParseError::Document { code: "unreadable-content-list".into() })?;
        let Some(items) = content.as_array() else {
            return Err(ParseError::Document { code: "invalid-content-list".into() });
        };

        // One POST is one task, so `page_idx` is already absolute and the
        // crops are already unique — the rebasing and renaming the cloud
        // client does exist only because it splits a document across tasks.
        let source_images = content_path.parent().unwrap_or(&extracted).join("images");
        let (pages, image_count) =
            render::render(items, total, &source_images, images_dir, images_rel)?;

        on_progress(Progress { pages_done: total, total_pages: total, backend: BACKEND });
        Ok(ParseOutput::new(pdf, total, pages, Some(BACKEND.to_string()), image_count))
    }

    /// The handshake, which on this side is finally talking to something that
    /// can disagree.
    ///
    /// `parser_version` is ours by construction — the content list is turned
    /// into page records by `render`, in this process, so a local parse writes
    /// exactly the artifacts this app reads, and a number from the server
    /// would be a number about MinerU rather than about the record shape. The
    /// skew that actually bites is the *API*: a server that speaks V1 cannot
    /// be talked into `/file_parse` by any version negotiation, which is why
    /// `probe` names it separately and the settings page shows it.
    fn health(&self) -> Health {
        Health {
            backend: BACKEND.to_string(),
            parser_version: PARSER_VERSION,
            // A server that is not running fails `preflight` as `NotReady` —
            // retryable, so the sweep comes back once it is up, and the file
            // is never marked permanently broken for it. The `Offline` in
            // `post_file_parse` is the same judgement about one request.
            ready: matches!(probe(&self.base_url), LocalHealth::Ready),
        }
    }
}

// ── Probing ──────────────────────────────────────────────────────────────────

/// What is answering at an address. Three real states plus the one that is
/// worth spelling out, because "unreachable" would be a lie about a MinerU
/// that is running perfectly and simply speaks the other API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalHealth {
    Ready,
    /// Answered `/health` and said it is not serving work.
    NotServing,
    /// No `/health`, but `/v1/health` answers: MinerU 4's V1 service, which
    /// dropped `/file_parse` for an upload/job/download cycle.
    WrongApi,
    Unreachable,
}

/// Ask an address what it is. Used by `health()` before every parse and by the
/// settings page's test button, which is why it takes a URL rather than a
/// client: the endpoint field has to be testable before it is saved.
pub fn probe(base_url: &str) -> LocalHealth {
    let base = base_url.trim().trim_end_matches('/');
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout(HEALTH_TIMEOUT)
        .build();
    match agent.get(&format!("{base}{HEALTH_PATH}")).call() {
        Ok(response) => {
            let body = response
                .into_string()
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .unwrap_or(Value::Null);
            match body.get("status").and_then(Value::as_str) {
                Some("healthy") => LocalHealth::Ready,
                _ => LocalHealth::NotServing,
            }
        }
        // No `/health` at all is the interesting case, because MinerU 4's V1
        // service is the likely thing on the other end and it is *running*.
        Err(ureq::Error::Status(404, _)) => {
            match agent.get(&format!("{base}{V1_HEALTH_PATH}")).call() {
                Ok(_) => LocalHealth::WrongApi,
                Err(_) => LocalHealth::Unreachable,
            }
        }
        // MinerU's own 503 is "the task manager is not up yet" — a server that
        // is there and will be ready shortly, which is not the same as absent.
        Err(ureq::Error::Status(_, _)) => LocalHealth::NotServing,
        Err(ureq::Error::Transport(_)) => LocalHealth::Unreachable,
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Only 4xx is about this document — MinerU answers 409 when the parse task
/// itself failed. Everything else is about the server or the address it was
/// reached at.
///
/// The difference that matters is permanence, not blame. `Document` is not
/// retryable, so anything mapped to it is a PDF the background sweep will
/// never pick up again. A 500 is the service falling over while it happened to
/// be this file's turn, and a 3xx is a proxy or a misspelled `engineUrl` —
/// neither says anything about the PDF, and both are fixed by someone doing
/// something elsewhere. `NotReady` is retryable, so the next sweep tries again
/// once they have.
fn http_failure(status: u16) -> ParseError {
    if (400..500).contains(&status) {
        ParseError::Document { code: format!("local-http-{status}") }
    } else {
        ParseError::NotReady { backend: BACKEND.to_string() }
    }
}

/// The multipart envelope: everything before the file's bytes, and everything
/// after. Split rather than concatenated so the PDF can be streamed between
/// the two halves instead of copied into a buffer.
fn envelope(boundary: &str, filename: &str) -> (Vec<u8>, Vec<u8>) {
    let mut head = String::new();
    for (name, value) in FIELDS {
        head.push_str(&format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        ));
    }
    // `files` is a list on the server's side, so one part is a list of one.
    head.push_str(&format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{}\"\r\n\
         Content-Type: application/pdf\r\n\r\n",
        header_safe(filename)
    ));
    (head.into_bytes(), format!("\r\n--{boundary}--\r\n").into_bytes())
}

/// A file name goes into a quoted header value, and the library's names are
/// whatever Canvas called them. The three characters that could close the
/// quote or end the header early are dropped rather than escaped: RFC 2183
/// escaping is read inconsistently, and the name is only used to label the
/// result directory inside the ZIP, which is then globbed for.
fn header_safe(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '"' | '\\' | '\r' | '\n')).collect()
}

/// A boundary only has to be absent from the body and unique among concurrent
/// requests; a timestamp, the process and a counter give both without needing
/// a random source.
fn boundary() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default();
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("oculus{nanos:016x}{:08x}{sequence:08x}", std::process::id())
}

/// A working directory for the result archive, removed however this parse
/// leaves. Not the cloud client's `Scratch`: that one names itself after a
/// batch, which a single POST has no equivalent of.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new() -> Result<Self, ParseError> {
        let root = std::env::temp_dir().join(format!("mineru-local-{}", boundary()));
        fs::create_dir_all(&root)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", root.display())))?;
        Ok(Self { root })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    // ── A MinerU that is not MinerU ──────────────────────────────────────────
    //
    // The same shape as `client`'s fixture: a `tiny_http` server on loopback,
    // driven through the real request. The base URL is configuration on this
    // side too, which is what makes that possible.

    struct Hit {
        method: String,
        url: String,
        content_type: String,
        body: Vec<u8>,
    }

    struct Fake {
        port: u16,
        hits: Arc<Mutex<Vec<Hit>>>,
        stop: Arc<AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl Fake {
        fn start<H>(handler: H) -> Self
        where
            H: Fn(&Hit) -> (u16, Vec<u8>) + Send + 'static,
        {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let port = server.server_addr().to_ip().unwrap().port();
            let hits: Arc<Mutex<Vec<Hit>>> = Arc::new(Mutex::new(Vec::new()));
            let stop = Arc::new(AtomicBool::new(false));
            let handle = {
                let hits = hits.clone();
                let stop = stop.clone();
                std::thread::spawn(move || {
                    while !stop.load(Ordering::SeqCst) {
                        let Ok(Some(mut request)) = server.recv_timeout(Duration::from_millis(20))
                        else {
                            continue;
                        };
                        let content_type = request
                            .headers()
                            .iter()
                            .find(|header| {
                                header.field.as_str().as_str().eq_ignore_ascii_case("content-type")
                            })
                            .map(|header| header.value.as_str().to_string())
                            .unwrap_or_default();
                        let mut body = Vec::new();
                        request.as_reader().read_to_end(&mut body).ok();
                        let hit = Hit {
                            method: request.method().as_str().to_string(),
                            url: request.url().to_string(),
                            content_type,
                            body,
                        };
                        let (status, payload) = handler(&hit);
                        hits.lock().unwrap().push(hit);
                        request
                            .respond(
                                tiny_http::Response::from_data(payload).with_status_code(status),
                            )
                            .ok();
                    }
                })
            };
            Self { port, hits, stop, handle: Some(handle) }
        }

        fn origin(&self) -> String {
            format!("http://127.0.0.1:{}", self.port)
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(handle) = self.handle.take() {
                handle.join().ok();
            }
        }
    }

    /// A port nothing is listening on: bound to learn the number, then closed.
    fn dead_origin() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        format!("http://127.0.0.1:{port}")
    }

    struct Dir {
        root: PathBuf,
    }

    impl Dir {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("oculus-local-{name}-{}", boundary()));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).ok();
        }
    }

    /// A real PDF with `pages` empty pages — `page_count` reads it with
    /// `lopdf`, so a stub file would not do.
    fn write_pdf(path: &Path, pages: usize) {
        use lopdf::{dictionary, Document, Object};
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let kids: Vec<Object> = (0..pages)
            .map(|_| document.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id }).into())
            .collect();
        let count = kids.len() as i64;
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
        );
        let catalog =
            document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        document.trailer.set("Root", catalog);
        document.save(path).unwrap();
    }

    /// The archive MinerU answers with: `{stem}/{parse_dir}/…`, which is why
    /// the content list is globbed for rather than named.
    fn result_zip(stem: &str, content: Value) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut archive = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            archive
                .start_file(format!("{stem}/auto/{stem}_content_list.json"), options)
                .unwrap();
            archive.write_all(content.to_string().as_bytes()).unwrap();
            archive.finish().unwrap();
        }
        buffer.into_inner()
    }

    // ── The request ──────────────────────────────────────────────────────────

    #[test]
    fn the_envelope_brackets_the_file_with_the_boundary() {
        let (head, tail) = envelope("BOUND", "Lecture \"3\".pdf");
        let head = String::from_utf8(head).unwrap();
        assert!(head.starts_with("--BOUND\r\n"), "{head}");
        // The quotes in the name would have closed the header value early.
        assert!(head.contains("filename=\"Lecture 3.pdf\""), "{head}");
        assert!(head.ends_with("\r\n\r\n"), "{head}");
        assert_eq!(String::from_utf8(tail).unwrap(), "\r\n--BOUND--\r\n");
    }

    #[test]
    fn the_post_carries_every_flag_and_the_file_itself() {
        let dir = Dir::new("request");
        let pdf = dir.root.join("Lecture 3.pdf");
        write_pdf(&pdf, 2);
        let bytes = fs::read(&pdf).unwrap();

        let content = json!([{ "type": "text", "text": "Hello", "page_idx": 0 }]);
        let zip = result_zip("Lecture 3", content);
        let server = Fake::start(move |_| (200, zip.clone()));

        let client = MinerULocal::new(&server.origin());
        let archive = dir.root.join("out.zip");
        client.post_file_parse(&pdf, &archive).unwrap();

        let hits = server.hits.lock().unwrap();
        let hit = hits.first().expect("nothing was posted");
        assert_eq!(hit.method, "POST");
        assert_eq!(hit.url, "/file_parse");

        let boundary = hit
            .content_type
            .split("boundary=")
            .nth(1)
            .expect(&hit.content_type)
            .to_string();
        let body = String::from_utf8_lossy(&hit.body);
        assert!(body.starts_with(&format!("--{boundary}\r\n")), "{}", &body[..80]);
        assert!(body.ends_with(&format!("\r\n--{boundary}--\r\n")));

        for (name, value) in FIELDS {
            assert!(
                body.contains(&format!("name=\"{name}\"\r\n\r\n{value}\r\n")),
                "{name} is not in the body"
            );
        }
        assert!(body.contains("name=\"files\"; filename=\"Lecture 3.pdf\""), "no file part");
        // The bytes went through untouched, between the two halves.
        assert!(
            hit.body.windows(bytes.len()).any(|window| window == bytes),
            "the PDF is not in the body"
        );
    }

    // ── The result ───────────────────────────────────────────────────────────

    #[test]
    fn the_zip_becomes_page_records_through_the_shared_renderer() {
        let dir = Dir::new("render");
        let pdf = dir.root.join("Lecture 3.pdf");
        write_pdf(&pdf, 3);

        // A trailing blank page is why `total_pages` is counted from the PDF:
        // the content list has nothing on page 3 to be counted by.
        let content = json!([
            { "type": "text", "text": "First slide", "page_idx": 0 },
            { "type": "text", "text": "Second slide", "page_idx": 1 },
        ]);
        let zip = result_zip("Lecture 3", content);
        let server = Fake::start(move |_| (200, zip.clone()));

        let images = dir.root.join("Lecture 3_images");
        let seen: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
        let output = MinerULocal::new(&server.origin())
            .parse(&pdf, &images, "Lecture 3_images", &|progress| {
                seen.lock().unwrap().push((progress.pages_done, progress.total_pages));
            })
            .unwrap();

        assert_eq!(output.page_count, 3);
        assert_eq!(output.pages.len(), 3);
        assert_eq!(output.pages.iter().map(|page| page.page_no).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert!(output.pages[0].markdown.contains("First slide"), "{:?}", output.pages[0]);
        assert!(output.pages[1].markdown.contains("Second slide"), "{:?}", output.pages[1]);
        assert_eq!(output.pages[2].markdown, "");
        assert_eq!(output.backend.as_deref(), Some(BACKEND));
        assert_eq!(output.parser_version, PARSER_VERSION);

        // The known page count, then the finish — and nothing invented in
        // between, which is the whole of what this endpoint can honestly say.
        assert_eq!(*seen.lock().unwrap(), vec![(0, 3), (3, 3)]);
    }

    #[test]
    fn a_result_that_is_not_an_archive_condemns_only_this_document() {
        let dir = Dir::new("garbage");
        let pdf = dir.root.join("Lecture 3.pdf");
        write_pdf(&pdf, 1);
        let server = Fake::start(|_| (200, b"not a zip at all".to_vec()));

        let error = MinerULocal::new(&server.origin())
            .parse(&pdf, &dir.root.join("images"), "images", &|_| {})
            .unwrap_err();
        assert_eq!(error.kind(), "document");
        assert!(!error.latching());
    }

    #[test]
    fn an_archive_with_no_content_list_is_a_document_failure() {
        let dir = Dir::new("empty-zip");
        let pdf = dir.root.join("Lecture 3.pdf");
        write_pdf(&pdf, 1);

        let mut buffer = Cursor::new(Vec::new());
        zip::ZipWriter::new(&mut buffer).finish().unwrap();
        let zip = buffer.into_inner();
        let server = Fake::start(move |_| (200, zip.clone()));

        let error = MinerULocal::new(&server.origin())
            .parse(&pdf, &dir.root.join("images"), "images", &|_| {})
            .unwrap_err();
        assert!(matches!(&error, ParseError::Document { code } if code == "no-content-list"));
    }

    // ── Failure ──────────────────────────────────────────────────────────────

    #[test]
    fn a_refused_connection_is_offline_not_a_broken_document() {
        let dir = Dir::new("offline");
        let pdf = dir.root.join("Lecture 3.pdf");
        write_pdf(&pdf, 1);

        let error = MinerULocal::new(&dead_origin())
            .parse(&pdf, &dir.root.join("images"), "images", &|_| {})
            .unwrap_err();
        assert_eq!(error.kind(), "offline");
        assert!(error.retryable());
        // Nothing local can exhaust a quota or have a token refused, which is
        // the point of the whole engine.
        assert!(!error.latching());
    }

    #[test]
    fn a_4xx_is_this_document_and_a_5xx_is_the_server() {
        let dir = Dir::new("http");
        let pdf = dir.root.join("Lecture 3.pdf");
        write_pdf(&pdf, 1);

        // 409 is what MinerU answers when the parse task itself failed.
        let refused = Fake::start(|_| (409, b"{\"detail\":\"http://signed.example\"}".to_vec()));
        let error = MinerULocal::new(&refused.origin())
            .parse(&pdf, &dir.root.join("images"), "images", &|_| {})
            .unwrap_err();
        assert!(matches!(&error, ParseError::Document { code } if code == "local-http-409"));
        // The server's body never reaches the student.
        assert!(!error.to_string().contains("signed.example"), "{error}");

        let broken = Fake::start(|_| (500, Vec::new()));
        let error = MinerULocal::new(&broken.origin())
            .parse(&pdf, &dir.root.join("images"), "images", &|_| {})
            .unwrap_err();
        assert_eq!(error.kind(), "not_ready");
        // The one that matters: a server fault must not condemn the PDF.
        assert!(error.retryable(), "a 500 is the server's, and the file deserves another go");
    }

    #[test]
    fn a_redirect_is_not_a_result_and_not_a_broken_document() {
        // `ureq` hands a 3xx back as `Ok`, because it will not replay an
        // unsized body — so without a status check this response's HTML is
        // written as the result ZIP and the PDF is condemned `Document`, which
        // never retries. A proxy in front of the server must not be able to
        // permanently break a file.
        let dir = Dir::new("redirect");
        let pdf = dir.root.join("Lecture 4.pdf");
        write_pdf(&pdf, 1);

        let moved = Fake::start(|_| (302, b"<html>moved</html>".to_vec()));
        let error = MinerULocal::new(&moved.origin())
            .parse(&pdf, &dir.root.join("images"), "images", &|_| {})
            .unwrap_err();
        assert_eq!(error.kind(), "not_ready", "a redirect says nothing about the PDF");
        assert!(error.retryable(), "fixing the address must be enough to recover the file");
    }

    // ── Probing ──────────────────────────────────────────────────────────────

    #[test]
    fn health_reads_minerus_own_status_word() {
        let healthy =
            Fake::start(|_| (200, json!({ "status": "healthy", "version": "3.4.5" }).to_string().into_bytes()));
        assert_eq!(probe(&healthy.origin()), LocalHealth::Ready);
        assert!(MinerULocal::new(&healthy.origin()).health().ready);

        let starting = Fake::start(|_| (503, json!({ "status": "unhealthy" }).to_string().into_bytes()));
        assert_eq!(probe(&starting.origin()), LocalHealth::NotServing);

        assert_eq!(probe(&dead_origin()), LocalHealth::Unreachable);
    }

    /// The skew that actually happens: MinerU 4's V1 service is up, healthy
    /// and useless to this client, because `/file_parse` is gone. Reporting
    /// that as "unreachable" would send someone to check a port that is fine.
    #[test]
    fn a_v1_server_is_named_rather_than_called_unreachable() {
        let v1 = Fake::start(|hit| {
            if hit.url == V1_HEALTH_PATH {
                (200, json!({ "status": "ok" }).to_string().into_bytes())
            } else {
                (404, Vec::new())
            }
        });
        assert_eq!(probe(&v1.origin()), LocalHealth::WrongApi);
        assert!(!MinerULocal::new(&v1.origin()).health().ready);
    }

    // ── The queue ────────────────────────────────────────────────────────────

    /// The regression the batcher covers on the other side and nothing covered
    /// here: `sync.rs` spawns a thread per PDF, so "one request at a time" has
    /// to be enforced somewhere or a full library arrives at once.
    ///
    /// `Fake` cannot show this — it serves one request at a time itself, so it
    /// would pass with the gate deleted. This listener answers each connection
    /// on its own thread and counts how many are ever in flight together.
    #[test]
    fn a_second_parse_waits_for_the_first() {
        use std::io::BufRead;
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let peak = Arc::new(AtomicU64::new(0));
        let live = Arc::new(AtomicU64::new(0));

        let served = {
            let (peak, live) = (peak.clone(), live.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming().take(2) {
                    let (peak, live) = (peak.clone(), live.clone());
                    std::thread::spawn(move || {
                        let mut stream = stream.unwrap();
                        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                        let mut length = 0usize;
                        loop {
                            let mut line = String::new();
                            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                                break;
                            }
                            if let Some(value) = line
                                .to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(str::trim)
                                .and_then(|v| v.parse::<usize>().ok())
                            {
                                length = value;
                            }
                        }
                        std::io::copy(&mut reader.by_ref().take(length as u64), &mut std::io::sink())
                            .unwrap();

                        // In flight from here to the response.
                        let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(150));
                        live.fetch_sub(1, Ordering::SeqCst);

                        let body = result_zip("Doc", json!([]));
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        stream.write_all(head.as_bytes()).unwrap();
                        stream.write_all(&body).unwrap();
                        stream.flush().ok();
                    });
                }
            })
        };

        let origin = format!("http://127.0.0.1:{port}");
        let dir = Dir::new("queue");
        let threads: Vec<_> = (0..2)
            .map(|n| {
                let (origin, root) = (origin.clone(), dir.root.clone());
                std::thread::spawn(move || {
                    let pdf = root.join(format!("Doc{n}.pdf"));
                    write_pdf(&pdf, 1);
                    let images = root.join(format!("images{n}"));
                    MinerULocal::new(&origin).parse(&pdf, &images, "images", &|_| {}).unwrap();
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        served.join().ok();

        assert_eq!(peak.load(Ordering::SeqCst), 1, "both parses were in flight at once");
    }
}

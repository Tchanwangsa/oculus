//! MinerU running on this machine, reached over loopback.
//!
//! MinerU's own server offers `POST /file_parse` — one multipart request in,
//! one result ZIP out — so none of the cloud client's batching, ledger or
//! keychain applies. The shared part is `render`, which turns the content list
//! into page records whichever side produced it; `FIELDS` match the cloud
//! client's parameters so the same PDF renders the same markdown.
//!
//! No quota, no token, and (the seam's rule) no server text in any error.

use std::fs;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

use crate::parse::{
    parse_config, Health, ParseError, ParseOutput, Parser, Progress, PARSER_VERSION,
};
use crate::ratelimit::transport_detail;

use super::client::{page_count, safe_extract};
use super::{render, WorkDir};

/// The `backend` stamped into every record this client writes.
pub const BACKEND: &str = "mineru-local";

// MinerU 3.x's synchronous parse endpoint and health check. `/v1/health` is
// MinerU 4's V1 service, which dropped `/file_parse`; `probe` asks for it only
// to name it.
const PARSE_PATH: &str = "/file_parse";
const HEALTH_PATH: &str = "/health";
const V1_HEALTH_PATH: &str = "/v1/health";

/// **A connect timeout and no read timeout**: a local parse is minutes of
/// silence, not a hang (see `docs/parsing.md`).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// The health check is a status line, so it does time out.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(3);

const COPY_CHUNK: usize = 1024 * 1024;

/// **One local parse at a time.** `sync.rs` spawns a thread per PDF; on the
/// cloud `Batcher` absorbs that, here nothing would, and a full sync would
/// park a socket per PDF on a server that serves one request at a time
/// (`mineru-api` reports `max_concurrent_requests: 1`). Poisoning is stepped
/// over so one panicked parse cannot disable the engine.
static PARSE_GATE: Mutex<()> = Mutex::new(());

/// The form fields: `backend`/`lang_list` are the cloud's
/// `model_version`/`language` (see `client::run_batch`). `return_content_list`
/// is load-bearing — the flat `.md` has no page boundaries.
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
    /// The client the app uses: the API root from the settings row. Infallible:
    /// a server that is not running is the first request's `Offline`.
    pub fn from_config() -> Self {
        Self::new(&parse_config().base_url)
    }

    pub fn new(base_url: &str) -> Self {
        Self { base_url: base_url.trim().trim_end_matches('/').to_string() }
    }

    /// One multipart POST, streamed. `ureq` 2 has no multipart, so the head,
    /// the open file and the closing boundary are chained into one reader with
    /// an explicit length (no chunked encoding).
    fn post_file_parse(&self, pdf: &Path, destination: &Path) -> Result<(), ParseError> {
        let file = fs::File::open(pdf)
            .map_err(|e| ParseError::Io(format!("open {}: {e}", pdf.display())))?;
        let size = file
            .metadata()
            .map_err(|e| ParseError::Io(format!("stat {}: {e}", pdf.display())))?
            .len();
        let name = pdf.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

        // Held to the end: the result ZIP is read off the same socket.
        let _permit = PARSE_GATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        let boundary = boundary();
        let (head, tail) = envelope(&boundary, &name);
        let length = head.len() as u64 + size + tail.len() as u64;
        // `.take(size)` keeps `Content-Length` true if a sync rewrites the
        // file mid-send; overflow would poison the pooled connection.
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
            // **A 3xx arrives as `Ok`**: `ureq` will not replay an unsized
            // body. Without this arm a proxy's page would be read as the ZIP
            // and the PDF condemned `Document`, which never retries.
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
        // Flushed by hand: `BufWriter`'s `Drop` flush swallows the error, and a
        // truncated archive would read as a permanent `Document` failure.
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
        // From the PDF, not the content list: a blank last page has no item.
        let total = page_count(pdf)?;

        // `/file_parse` blocks with nothing to subscribe to, so progress is the
        // page count and then the finish — never an estimate in between.
        on_progress(Progress { pages_done: 0, total_pages: total, backend: BACKEND });

        let scratch = WorkDir::new(format!("mineru-local-{}", boundary()))?;
        let archive = scratch.path().join("result.zip");
        let extracted = scratch.path().join("result");
        self.post_file_parse(pdf, &archive)?;
        safe_extract(&archive, &extracted)?;

        let (content_path, items) = super::content_list(&extracted)?;

        // One POST is one task: `page_idx` is absolute, crops are unique.
        let source_images = content_path.parent().unwrap_or(&extracted).join("images");
        let (pages, image_count) =
            render::render(&items, total, &source_images, images_dir, images_rel)?;

        on_progress(Progress { pages_done: total, total_pages: total, backend: BACKEND });
        Ok(ParseOutput::new(pdf, total, pages, Some(BACKEND.to_string()), image_count))
    }

    /// `parser_version` is ours by construction (`render` runs here); the skew
    /// that bites is the API, which `probe` names. Not running is `NotReady`,
    /// which retries.
    fn health(&self) -> Health {
        Health {
            backend: BACKEND.to_string(),
            parser_version: PARSER_VERSION,
            ready: matches!(probe(&self.base_url), LocalHealth::Ready),
        }
    }
}

// ── Probing ──────────────────────────────────────────────────────────────────

/// What is answering at an address. `WrongApi` is spelled out because
/// "unreachable" would be a lie about a running MinerU 4.
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

/// Ask an address what it is. Takes a URL, not a client, so the settings
/// page can test the endpoint field before it is saved.
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
        Err(ureq::Error::Status(404, _)) => {
            match agent.get(&format!("{base}{V1_HEALTH_PATH}")).call() {
                Ok(_) => LocalHealth::WrongApi,
                Err(_) => LocalHealth::Unreachable,
            }
        }
        // MinerU's 503 is "task manager not up yet": present, not absent.
        Err(ureq::Error::Status(_, _)) => LocalHealth::NotServing,
        Err(ureq::Error::Transport(_)) => LocalHealth::Unreachable,
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Only 4xx is about this document (MinerU answers 409 when the parse task
/// failed). A 5xx or 3xx is the server or the address, so it maps to the
/// retryable `NotReady` rather than the permanent `Document`.
fn http_failure(status: u16) -> ParseError {
    if (400..500).contains(&status) {
        ParseError::Document { code: format!("local-http-{status}") }
    } else {
        ParseError::NotReady { backend: BACKEND.to_string() }
    }
}

/// The multipart envelope, split so the PDF streams between the two halves.
fn envelope(boundary: &str, filename: &str) -> (Vec<u8>, Vec<u8>) {
    let mut head = String::new();
    for (name, value) in FIELDS {
        head.push_str(&format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        ));
    }
    head.push_str(&format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{}\"\r\n\
         Content-Type: application/pdf\r\n\r\n",
        header_safe(filename)
    ));
    (head.into_bytes(), format!("\r\n--{boundary}--\r\n").into_bytes())
}

/// Drop the characters that could end a quoted header value early. Dropped,
/// not escaped: RFC 2183 escaping is read inconsistently, and the name only
/// labels a directory inside the ZIP.
fn header_safe(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '"' | '\\' | '\r' | '\n')).collect()
}

/// Unique among concurrent requests without a random source.
fn boundary() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nanos = crate::clock::now_nanos() as u64;
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("oculus{nanos:016x}{:08x}{sequence:08x}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::test_support::{dead_origin, write_pdf, FakeServer, Scratch};

    use serde_json::json;

    /// The archive MinerU answers with: `{stem}/{parse_dir}/…`.
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
        assert!(head.contains("filename=\"Lecture 3.pdf\""), "{head}");
        assert!(head.ends_with("\r\n\r\n"), "{head}");
        assert_eq!(String::from_utf8(tail).unwrap(), "\r\n--BOUND--\r\n");
    }

    #[test]
    fn the_post_carries_every_flag_and_the_file_itself() {
        let dir = Scratch::new("local-request");
        let pdf = dir.join("Lecture 3.pdf");
        write_pdf(&pdf, 2);
        let bytes = fs::read(&pdf).unwrap();

        let content = json!([{ "type": "text", "text": "Hello", "page_idx": 0 }]);
        let zip = result_zip("Lecture 3", content);
        let server = FakeServer::start(move |_| (200, zip.clone()));

        let client = MinerULocal::new(&server.origin());
        let archive = dir.join("out.zip");
        client.post_file_parse(&pdf, &archive).unwrap();

        let hits = server.hits();
        let hit = hits.first().expect("nothing was posted");
        assert_eq!(hit.method, "POST");
        assert_eq!(hit.url, "/file_parse");

        let content_type = hit.header("content-type").unwrap_or_default();
        let boundary = content_type.split("boundary=").nth(1).expect(content_type).to_string();
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
        assert!(
            hit.body.windows(bytes.len()).any(|window| window == bytes),
            "the PDF is not in the body"
        );
    }

    // ── The result ───────────────────────────────────────────────────────────

    #[test]
    fn the_zip_becomes_page_records_through_the_shared_renderer() {
        let dir = Scratch::new("local-render");
        let pdf = dir.join("Lecture 3.pdf");
        write_pdf(&pdf, 3);

        // A trailing blank page: nothing on page 3 to be counted by.
        let content = json!([
            { "type": "text", "text": "First slide", "page_idx": 0 },
            { "type": "text", "text": "Second slide", "page_idx": 1 },
        ]);
        let zip = result_zip("Lecture 3", content);
        let server = FakeServer::start(move |_| (200, zip.clone()));

        let images = dir.join("Lecture 3_images");
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

        assert_eq!(*seen.lock().unwrap(), vec![(0, 3), (3, 3)]);
    }

    #[test]
    fn a_result_that_is_not_an_archive_condemns_only_this_document() {
        let dir = Scratch::new("local-garbage");
        let pdf = dir.join("Lecture 3.pdf");
        write_pdf(&pdf, 1);
        let server = FakeServer::start(|_| (200, b"not a zip at all".to_vec()));

        let error = MinerULocal::new(&server.origin())
            .parse(&pdf, &dir.join("images"), "images", &|_| {})
            .unwrap_err();
        assert_eq!(error.kind(), "document");
        assert!(!error.latching());
    }

    #[test]
    fn an_archive_with_no_content_list_is_a_document_failure() {
        let dir = Scratch::new("local-empty-zip");
        let pdf = dir.join("Lecture 3.pdf");
        write_pdf(&pdf, 1);

        let mut buffer = Cursor::new(Vec::new());
        zip::ZipWriter::new(&mut buffer).finish().unwrap();
        let zip = buffer.into_inner();
        let server = FakeServer::start(move |_| (200, zip.clone()));

        let error = MinerULocal::new(&server.origin())
            .parse(&pdf, &dir.join("images"), "images", &|_| {})
            .unwrap_err();
        assert!(matches!(&error, ParseError::Document { code } if code == "no-content-list"));
    }

    // ── Failure ──────────────────────────────────────────────────────────────

    #[test]
    fn a_refused_connection_is_offline_not_a_broken_document() {
        let dir = Scratch::new("local-offline");
        let pdf = dir.join("Lecture 3.pdf");
        write_pdf(&pdf, 1);

        let error = MinerULocal::new(&dead_origin())
            .parse(&pdf, &dir.join("images"), "images", &|_| {})
            .unwrap_err();
        assert_eq!(error.kind(), "offline");
        assert!(error.retryable());
        assert!(!error.latching());
    }

    #[test]
    fn a_4xx_is_this_document_and_a_5xx_is_the_server() {
        let dir = Scratch::new("local-http");
        let pdf = dir.join("Lecture 3.pdf");
        write_pdf(&pdf, 1);

        let refused = FakeServer::start(|_| (409, b"{\"detail\":\"http://signed.example\"}".to_vec()));
        let error = MinerULocal::new(&refused.origin())
            .parse(&pdf, &dir.join("images"), "images", &|_| {})
            .unwrap_err();
        assert!(matches!(&error, ParseError::Document { code } if code == "local-http-409"));
        assert!(!error.to_string().contains("signed.example"), "{error}");

        let broken = FakeServer::start(|_| (500, Vec::new()));
        let error = MinerULocal::new(&broken.origin())
            .parse(&pdf, &dir.join("images"), "images", &|_| {})
            .unwrap_err();
        assert_eq!(error.kind(), "not_ready");
        assert!(error.retryable(), "a 500 is the server's, and the file deserves another go");
    }

    #[test]
    fn a_redirect_is_not_a_result_and_not_a_broken_document() {
        // See the 3xx arm in `post_file_parse`.
        let dir = Scratch::new("local-redirect");
        let pdf = dir.join("Lecture 4.pdf");
        write_pdf(&pdf, 1);

        let moved = FakeServer::start(|_| (302, b"<html>moved</html>".to_vec()));
        let error = MinerULocal::new(&moved.origin())
            .parse(&pdf, &dir.join("images"), "images", &|_| {})
            .unwrap_err();
        assert_eq!(error.kind(), "not_ready", "a redirect says nothing about the PDF");
        assert!(error.retryable(), "fixing the address must be enough to recover the file");
    }

    // ── Probing ──────────────────────────────────────────────────────────────

    #[test]
    fn health_reads_minerus_own_status_word() {
        let healthy =
            FakeServer::start(|_| (200, json!({ "status": "healthy", "version": "3.4.5" }).to_string().into_bytes()));
        assert_eq!(probe(&healthy.origin()), LocalHealth::Ready);
        assert!(MinerULocal::new(&healthy.origin()).health().ready);

        let starting = FakeServer::start(|_| (503, json!({ "status": "unhealthy" }).to_string().into_bytes()));
        assert_eq!(probe(&starting.origin()), LocalHealth::NotServing);

        assert_eq!(probe(&dead_origin()), LocalHealth::Unreachable);
    }

    #[test]
    fn a_v1_server_is_named_rather_than_called_unreachable() {
        let v1 = FakeServer::start(|hit| {
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

    /// `PARSE_GATE`. Not `FakeServer`, which serializes requests itself and would
    /// pass with the gate deleted: this listener serves each connection on its
    /// own thread and counts how many are in flight together.
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
        let dir = Scratch::new("local-queue");
        let threads: Vec<_> = (0..2)
            .map(|n| {
                let (origin, root) = (origin.clone(), dir.to_path_buf());
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

    // ── The real thing ───────────────────────────────────────────────────────

    /// A PDF that actually says something, for a server that actually reads it.
    /// One text object per line, so each `Td` is absolute rather than stacking.
    fn write_text_pdf(path: &Path, lines: &[&str]) {
        use lopdf::content::{Content, Operation};
        use lopdf::{dictionary, Document, Object, Stream};

        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let font = document.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let resources = document.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font },
        });

        let mut operations = Vec::new();
        for (n, line) in lines.iter().enumerate() {
            operations.push(Operation::new("BT", vec![]));
            operations.push(Operation::new("Tf", vec!["F1".into(), 28.into()]));
            operations.push(Operation::new(
                "Td",
                vec![72.into(), (700 - 60 * n as i64).into()],
            ));
            operations.push(Operation::new("Tj", vec![Object::string_literal(*line)]));
            operations.push(Operation::new("ET", vec![]));
        }
        let content = Content { operations };
        let stream = document.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => stream,
            "Resources" => resources,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
            }),
        );
        let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        document.trailer.set("Root", catalog);
        document.save(path).unwrap();
    }

    /// Against a real MinerU: the only check that `FIELDS` are spelled the
    /// way that server reads them. Ignored; needs a server:
    ///
    /// ```text
    /// uv tool install -U "mineru[core]>=3.4,<4"
    /// MINERU_API_OUTPUT_ROOT="$HOME/.cache/mineru-api" \
    ///   mineru-api --host 127.0.0.1 --port 8000
    /// cargo test --lib parse::mineru::local::tests::a_real_mineru -- --ignored --nocapture
    /// ```
    ///
    /// `OCULUS_MINERU_URL` moves the address; `OCULUS_MINERU_PDF` swaps in a
    /// real document; `OCULUS_MINERU_DUMP` writes the markdown and images out.
    #[test]
    #[ignore = "needs a MinerU 3.x server on 127.0.0.1:8000 — see the doc comment"]
    fn a_real_mineru_answers_the_way_this_client_expects() {
        let base =
            std::env::var("OCULUS_MINERU_URL")
                .unwrap_or_else(|_| crate::parse::LOCAL_BASE_URL.to_string());
        assert_eq!(probe(&base), LocalHealth::Ready, "no healthy MinerU at {base}");

        let dir = Scratch::new("local-live");
        let pdf = match std::env::var("OCULUS_MINERU_PDF") {
            Ok(path) => PathBuf::from(path),
            Err(_) => {
                let path = dir.join("Live.pdf");
                write_text_pdf(
                    &path,
                    &["Chapter One", "The quick brown fox", "jumps over the lazy dog."],
                );
                path
            }
        };

        let images = dir.join("Live_images");
        let output = MinerULocal::new(&base)
            .parse(&pdf, &images, "Live_images", &|progress| {
                eprintln!("  {}/{} pages", progress.pages_done, progress.total_pages);
            })
            .expect("the parse failed");

        assert_eq!(output.backend.as_deref(), Some(BACKEND));
        assert_eq!(output.parser_version, PARSER_VERSION);
        assert_eq!(output.pages.len(), output.page_count as usize);
        assert!(output.page_count > 0, "no pages");
        // At least one page with text proves the form fields landed.
        let written = output.pages.iter().filter(|p| !p.markdown.trim().is_empty()).count();
        assert!(written > 0, "every page came back empty — check the form fields");
        eprintln!(
            "{} pages, {written} with markdown, {} images",
            output.page_count, output.image_count
        );

        if let Ok(into) = std::env::var("OCULUS_MINERU_DUMP") {
            let into = PathBuf::from(into);
            fs::create_dir_all(&into).unwrap();
            for page in &output.pages {
                fs::write(
                    into.join(format!("page-{:03}.md", page.page_no)),
                    &page.markdown,
                )
                .unwrap();
            }
            if images.is_dir() {
                let copied = into.join("Live_images");
                fs::create_dir_all(&copied).unwrap();
                for entry in fs::read_dir(&images).unwrap().flatten() {
                    fs::copy(entry.path(), copied.join(entry.file_name())).unwrap();
                }
            }
            eprintln!("dumped to {}", into.display());
        }
    }
}

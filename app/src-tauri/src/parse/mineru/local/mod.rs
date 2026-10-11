//! MinerU running on this machine, reached over loopback.
//!
//! MinerU's own server offers `POST /file_parse` — one multipart request in,
//! one result ZIP out — so none of the cloud client's batching, ledger or
//! keychain applies. The shared part is `render`, which turns the content list
//! into page records whichever side produced it; `FIELDS` match the cloud
//! client's parameters so the same PDF renders the same markdown.
//!
//! No quota, no token, and (the seam's rule) no server text in any error.

mod probe;
mod request;
#[cfg(test)]
mod tests;

pub use probe::{probe, LocalHealth};
use request::{boundary, envelope, http_failure};

use std::fs;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use crate::parse::{
    check_skipped, parse_config, Health, ParseError, ParseOutput, Parser, Progress, PARSER_VERSION,
};
use crate::providers::ratelimit::transport_detail;

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

/// **One local parse at a time.** `sync/phases/output.rs` spawns a thread per PDF; on the
/// cloud `Batcher` absorbs that, here nothing would, and a full sync would
/// park a socket per PDF on a server that serves one request at a time
/// (`mineru-api` reports `max_concurrent_requests: 1`). Taken before the first
/// progress report, so a file waiting here still reads as queued. Poisoning is
/// stepped over so one panicked parse cannot disable the engine.
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
        Self {
            base_url: base_url.trim().trim_end_matches('/').to_string(),
        }
    }

    /// One multipart POST, streamed. `ureq` 2 has no multipart, so the head,
    /// the open file and the closing boundary are chained into one reader with
    /// an explicit length (no chunked encoding). The caller holds `PARSE_GATE`.
    fn post_file_parse(&self, pdf: &Path, destination: &Path) -> Result<(), ParseError> {
        let file = fs::File::open(pdf)
            .map_err(|e| ParseError::Io(format!("open {}: {e}", pdf.display())))?;
        let size = file
            .metadata()
            .map_err(|e| ParseError::Io(format!("stat {}: {e}", pdf.display())))?
            .len();
        let name = pdf
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let boundary = boundary();
        let (head, tail) = envelope(&boundary, &name);
        let length = head.len() as u64 + size + tail.len() as u64;
        // `.take(size)` keeps `Content-Length` true if a sync rewrites the
        // file mid-send; overflow would poison the pooled connection.
        let body = Cursor::new(head)
            .chain(BufReader::with_capacity(COPY_CHUNK, file).take(size))
            .chain(Cursor::new(tail));

        let agent = ureq::AgentBuilder::new()
            .timeout_connect(CONNECT_TIMEOUT)
            .build();
        let sent = agent
            .post(&format!("{}{PARSE_PATH}", self.base_url))
            .set(
                "Content-Type",
                &format!("multipart/form-data; boundary={boundary}"),
            )
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
        out.flush()
            .map_err(|e| ParseError::Io(format!("write {}: {e}", destination.display())))
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
        // A skip cannot abort the blocking request, so it is checked on each
        // side of it and a result that arrives after one is discarded.
        check_skipped(pdf)?;
        // From the PDF, not the content list: a blank last page has no item.
        let total = page_count(pdf)?;

        let scratch = WorkDir::new(format!("mineru-local-{}", boundary()))?;
        let archive = scratch.path().join("result.zip");
        let extracted = scratch.path().join("result");
        {
            // Held until the result ZIP is read off the socket.
            let _permit = PARSE_GATE
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            check_skipped(pdf)?;
            // `/file_parse` blocks with nothing to subscribe to, so progress is
            // the page count and then the finish — never an estimate between.
            // Loopback has no upload phase worth showing.
            on_progress(Progress::processing(0, total, BACKEND));
            self.post_file_parse(pdf, &archive)?;
        }
        check_skipped(pdf)?;
        safe_extract(&archive, &extracted)?;

        let (content_path, items) = super::content_list(&extracted)?;

        // One POST is one task: `page_idx` is absolute, crops are unique.
        let source_images = content_path.parent().unwrap_or(&extracted).join("images");
        let (pages, image_count) =
            render::render(&items, total, &source_images, images_dir, images_rel)?;

        on_progress(Progress::processing(total, total, BACKEND));
        Ok(ParseOutput::new(
            pdf,
            total,
            pages,
            Some(BACKEND.to_string()),
            image_count,
        ))
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

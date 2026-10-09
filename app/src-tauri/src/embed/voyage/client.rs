//! The Voyage multimodal protocol, and the `Embedder` the app indexes through.
//!
//! One `POST`, one answer. `output_dtype` is precision and has no `float16`;
//! `output_encoding: "base64"` is transport and returns f32 little-endian. The
//! f16 narrowing and re-normalising happen in `embed::pack_vector`, never here.
//!
//! Errors carry a code, never the server's text (see `EmbedError`). A 429 is
//! not a failure: it is honoured, learned from (the only place the account's
//! real limits are stated) and retried without consuming an attempt.
//!
//! The request goes through `oculus-keyd` when it is installed and the API
//! root is Voyage's own, so this process never holds the key; otherwise
//! straight to the API root with the keychain's key (`with_config`). Both
//! routes hand `send` the same `RawResponse`.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{json, Value};

use crate::credentials::{Credentialed, KeydError, RawResponse};
use crate::embed::raster::{self, RenderedPage};
use crate::embed::{
    embed_config, pack_vector, unpack_vector, CredentialSource, EmbedConfig, EmbedError,
    EmbedOutput, Embedder, Health, Limiter, Progress, Wait, CLOUD_BASE_URL, EMBED_DIM, EMBED_MODEL,
};

use crate::ratelimit::{nap, transport_detail, Retry};

use super::batch::{self, Limits, RequestRun};
use super::ledger::{is_about_credit, RateGate, UsageLedger};

/// The `backend` this client reports, and what `Progress` is stamped with.
pub const BACKEND: &str = "voyage-cloud";

/// `/embeddings` is the text-only model's and refuses images.
const EMBED_PATH: &str = "/multimodalembeddings";

/// `CLOUD_BASE_URL`'s path, which keyd's `forward` is given in place of a URL.
const CLOUD_PATH: &str = "/v1";

/// The wire side of `embed::QUERY_INSTRUCTION`.
const INPUT_TYPE_DOCUMENT: &str = "document";
const INPUT_TYPE_QUERY: &str = "query";

/// Attempts per request. A 429 deliberately does not consume one.
const ATTEMPTS: u32 = 4;
/// Generous: a full request is tens of MB of base64 page images.
const API_TIMEOUT: Duration = Duration::from_secs(300);

/// How long one request may spend throttled before it becomes a retryable
/// `RateLimited`. Minutes of 429s are routine, but a zero limit or a proxy
/// answering 429 to everything must not park a thread for ever.
const THROTTLE_DEADLINE: Duration = Duration::from_secs(30 * 60);

/// A pacing wait shorter than this is not reported: the row is still moving.
/// A 429's wait is always reported.
const REPORTED_WAIT: Duration = Duration::from_secs(2);

/// What one request has said about its waits, so the same wait seen twice
/// (the 429's nap, then the gate's pause it set) is one event, not two.
struct WaitNotice<'a> {
    on_wait: &'a dyn Fn(Option<Wait>),
    shown_until_ms: Option<u64>,
}

impl<'a> WaitNotice<'a> {
    fn new(on_wait: &'a dyn Fn(Option<Wait>)) -> Self {
        Self {
            on_wait,
            shown_until_ms: None,
        }
    }

    /// Reported unless it ends within the threshold of the one already shown.
    fn show(&mut self, wait: Wait) {
        let threshold = REPORTED_WAIT.as_millis() as u64;
        if let Some(shown) = self.shown_until_ms {
            if shown.abs_diff(wait.until_ms) <= threshold {
                return;
            }
        }
        self.shown_until_ms = Some(wait.until_ms);
        (self.on_wait)(Some(wait));
    }

    fn clear(&mut self) {
        if self.shown_until_ms.take().is_some() {
            (self.on_wait)(None);
        }
    }
}

/// What one request is about to cost, reserved before it is sent.
#[derive(Debug, Clone, Copy, Default)]
struct Cost {
    tokens: u64,
    pixels: u64,
}

/// Why `send` came back without vectors. `Resize` is not a failure: a 429
/// showed the account's ceiling is below this request, which no pace can fix,
/// so it is handed back to be repacked smaller.
enum SendFailure {
    Embed(EmbedError),
    Resize,
}

impl From<EmbedError> for SendFailure {
    fn from(error: EmbedError) -> Self {
        SendFailure::Embed(error)
    }
}

/// How a request gets its key.
#[derive(Clone)]
enum Auth {
    /// This process holds the key: keyd is absent, or the API root is not
    /// Voyage's.
    Direct(Arc<String>),
    /// keyd adds it; the key never enters this process.
    Keyd(Arc<Credentialed>),
}

/// A request that got no answer. `Transport` backs off and retries like a
/// dropped connection; `Fatal` ends the request.
enum Unanswered {
    Transport(String),
    Fatal(EmbedError),
}

impl Unanswered {
    /// keyd's refusals in the seam's vocabulary. `upstream` and a broken
    /// socket are a failure to reach Voyage, so they back off and retry.
    fn from_keyd(error: KeydError) -> Self {
        match error {
            KeydError::Missing(_) => Unanswered::Fatal(EmbedError::MissingCredentials),
            KeydError::Keychain(detail) => {
                Unanswered::Fatal(EmbedError::UnreadableCredentials(detail))
            }
            KeydError::Upstream(detail) | KeydError::Broken(detail) => {
                Unanswered::Transport(detail)
            }
            KeydError::Absent => Unanswered::Transport("oculus-keyd stopped listening".into()),
            other @ (KeydError::Request(_) | KeydError::Caller(_) | KeydError::Vault(_)) => {
                Unanswered::Fatal(EmbedError::Broker(other.to_string()))
            }
        }
    }
}

#[derive(Clone)]
pub struct VoyageCloud {
    base_url: Arc<String>,
    auth: Auth,
    ledger: Arc<UsageLedger>,
    gate: Arc<RateGate>,
    limits: Limits,
    /// Every wait is multiplied by this; tests shrink it.
    time_scale: f64,
}

impl VoyageCloud {
    /// The client the app uses: engine and API root from the settings row, key
    /// from the keychain.
    pub fn from_config() -> Result<Self, EmbedError> {
        Self::with_config(&embed_config())
    }

    pub fn with_config(config: &EmbedConfig) -> Result<Self, EmbedError> {
        Self::with_config_in(config, &crate::paths::data_dir(), || {
            config.credentials.key()
        })
    }

    /// keyd when it is installed and `config` names Voyage's own API root, so
    /// keyd's fixed origin is the one meant; otherwise the key from
    /// `direct_key`. Only an absent keyd falls back. keyd's `has` keeps the
    /// early "no key saved" check.
    fn with_config_in(
        config: &EmbedConfig,
        data_dir: &Path,
        direct_key: impl FnOnce() -> Result<Option<String>, EmbedError>,
    ) -> Result<Self, EmbedError> {
        let cloud_root = config.base_url.trim_end_matches('/') == CLOUD_BASE_URL;
        if config.credentials == CredentialSource::Keychain && cloud_root {
            let broker = Credentialed::at(data_dir);
            match broker.has(crate::voyage::SECRET) {
                Ok(true) => return Ok(Self::through_keyd(broker)),
                Ok(false) => return Err(EmbedError::MissingCredentials),
                Err(KeydError::Absent) => {}
                Err(error) => {
                    return Err(match Unanswered::from_keyd(error) {
                        Unanswered::Fatal(error) => error,
                        Unanswered::Transport(detail) => EmbedError::Broker(detail),
                    })
                }
            }
        }
        let key = direct_key()?.unwrap_or_default();
        Self::new(&config.base_url, &key)
    }

    /// `base_url` is passed in so tests can point the protocol at a local server.
    pub fn new(base_url: &str, key: &str) -> Result<Self, EmbedError> {
        let key = key.trim();
        if key.is_empty() {
            return Err(EmbedError::MissingCredentials);
        }
        Ok(Self::with_auth(
            base_url,
            Auth::Direct(Arc::new(key.to_string())),
        ))
    }

    fn through_keyd(broker: Credentialed) -> Self {
        Self::with_auth(CLOUD_BASE_URL, Auth::Keyd(Arc::new(broker)))
    }

    fn with_auth(base_url: &str, auth: Auth) -> Self {
        Self {
            base_url: Arc::new(base_url.trim_end_matches('/').to_string()),
            auth,
            ledger: UsageLedger::shared(),
            gate: RateGate::shared(),
            limits: Limits::default(),
            time_scale: 1.0,
        }
    }

    pub fn with_ledger(mut self, ledger: Arc<UsageLedger>) -> Self {
        self.ledger = ledger;
        self
    }

    pub fn with_gate(mut self, gate: Arc<RateGate>) -> Self {
        self.gate = gate;
        self
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    pub fn with_time_scale(mut self, scale: f64) -> Self {
        self.time_scale = scale;
        self
    }

    // ── One request ──────────────────────────────────────────────────────────

    /// One POST of `body` by this client's route. A status is an answer,
    /// whatever it is; `send` reads it.
    fn post(&self, body: &[u8]) -> Result<RawResponse, Unanswered> {
        let headers = [
            ("Content-Type", "application/json"),
            ("Accept", "application/json"),
        ];
        match &self.auth {
            Auth::Keyd(broker) => broker
                .send(
                    crate::voyage::SECRET,
                    "POST",
                    &format!("{CLOUD_PATH}{EMBED_PATH}"),
                    &headers,
                    body,
                    Some(API_TIMEOUT),
                )
                .map_err(Unanswered::from_keyd),
            Auth::Direct(key) => {
                let mut request = ureq::post(&format!("{}{}", self.base_url, EMBED_PATH))
                    .timeout(API_TIMEOUT)
                    .set("Authorization", &format!("Bearer {key}"));
                for (name, value) in headers {
                    request = request.set(name, value);
                }
                let response = match request.send_bytes(body) {
                    Ok(response) | Err(ureq::Error::Status(_, response)) => response,
                    Err(ureq::Error::Transport(transport)) => {
                        return Err(Unanswered::Transport(transport_detail(&transport)))
                    }
                };
                let status = response.status();
                let headers = response
                    .headers_names()
                    .into_iter()
                    .filter_map(|name| {
                        let value = response.header(&name)?.to_string();
                        Some((name, value))
                    })
                    .collect();
                let body = response.into_string().unwrap_or_default().into_bytes();
                Ok(RawResponse {
                    status,
                    headers,
                    body,
                })
            }
        }
    }

    /// Send one request and hand back one vector per input: reserve, wait for
    /// the gate, send, and on a 429 learn the tier and go round again.
    /// `on_wait` hears of each rate-limit wait long enough to look like a
    /// stall, and `None` once the request is admitted.
    fn send(
        &self,
        inputs: Value,
        input_type: &str,
        expected: usize,
        cost: Cost,
        on_wait: &dyn Fn(Option<Wait>),
    ) -> Result<Vec<Vec<f32>>, SendFailure> {
        let body = json!({
            "model": EMBED_MODEL,
            "inputs": inputs,
            "input_type": input_type,
            "output_dimension": EMBED_DIM,
            "output_encoding": "base64",
        });
        let encoded = serde_json::to_vec(&body)
            .map_err(|e| SendFailure::Embed(EmbedError::Io(format!("encode request: {e}"))))?;

        // Reserved before sending and never given back: a request that dies in
        // flight may still have been billed.
        self.ledger.record(1, cost.tokens, cost.pixels)?;

        let mut retry = Retry::new(ATTEMPTS, self.time_scale);
        let throttled_until = Instant::now() + THROTTLE_DEADLINE.mul_f64(self.time_scale);
        // Reported by the deadline's error.
        #[allow(unused_assignments)]
        let mut last_retry_after: Option<u64> = None;
        let mut notice = WaitNotice::new(on_wait);

        while retry.attempts_left() {
            self.gate
                .admit_reporting(cost.tokens, &mut |wait, limiter| {
                    if wait > REPORTED_WAIT {
                        notice.show(Wait::after(wait, limiter));
                    }
                });
            notice.clear();
            let response = match self.post(&encoded) {
                Ok(response) => response,
                Err(Unanswered::Fatal(error)) => return Err(error.into()),
                Err(Unanswered::Transport(detail)) => {
                    if retry.back_off() {
                        continue;
                    }
                    return Err(EmbedError::Offline(detail).into());
                }
            };

            let status = response.status;
            if status >= 400 {
                let retry_after = response
                    .header("Retry-After")
                    .and_then(|value| value.trim().parse::<f64>().ok());
                let text = String::from_utf8_lossy(&response.body);

                if status == 429 {
                    // The body states the real limits; read, then dropped.
                    let wait = self.gate.throttled(retry_after, &text);
                    last_retry_after = Some(wait.as_secs());

                    // Livelock guard: over the per-minute ceiling no wait
                    // helps, so repack smaller. A single input cannot be
                    // split and takes the ordinary wait.
                    if expected > 1 && cost.tokens > self.max_tokens() {
                        return Err(SendFailure::Resize);
                    }
                    if Instant::now() >= throttled_until {
                        return Err(EmbedError::RateLimited {
                            retry_after_secs: last_retry_after,
                        }
                        .into());
                    }
                    // The gate's pause and drained buckets may outlast the nap.
                    let resume = wait
                        .mul_f64(self.time_scale)
                        .max(self.gate.expected_wait(cost.tokens));
                    notice.show(Wait::after(resume, Limiter::Throttled));
                    nap(wait, self.time_scale);
                    continue;
                }
                match self.status_error(status, &text, &mut retry) {
                    Some(error) => return Err(error.into()),
                    // A 5xx with attempts left: already backed off.
                    None => continue,
                }
            }

            let payload = match serde_json::from_slice::<Value>(&response.body) {
                Ok(payload) => payload,
                Err(_) if retry.back_off() => continue,
                Err(_) => return Err(EmbedError::Offline("unreadable response".into()).into()),
            };

            // What Voyage billed corrects the estimate; a calm response also
            // raises the throttle's ceiling.
            let billed = payload
                .get("usage")
                .and_then(|usage| usage.get("total_tokens"))
                .and_then(Value::as_u64);
            self.gate.succeeded(cost.tokens, billed);

            return decode_response(&payload, expected).map_err(SendFailure::Embed);
        }
        Err(EmbedError::Offline("request failed".into()).into())
    }

    /// Turn a non-429 status into the seam's vocabulary; `None` means "backed
    /// off, go round again". `body` only decides money vs a bad key and never
    /// reaches an error, a log or the UI.
    fn status_error(&self, status: u16, body: &str, retry: &mut Retry) -> Option<EmbedError> {
        // Money, not pace; the server's answer outranks the local ledger.
        if status == 402 || (matches!(status, 401 | 403) && is_about_credit(body)) {
            self.ledger.latch_exhausted();
            return Some(EmbedError::QuotaExhausted);
        }
        if matches!(status, 401 | 403) {
            let lowered = body.to_lowercase();
            return Some(EmbedError::RejectedCredentials {
                code: Some(status.to_string()),
                expired: lowered.contains("expired") || lowered.contains("revoked"),
            });
        }
        if status >= 500 {
            if retry.back_off() {
                return None;
            }
            // Not `Document`: that would fail the file for something transient.
            return Some(EmbedError::Offline(format!("http {status}")));
        }
        Some(EmbedError::Document {
            code: format!("http-{status}"),
        })
    }
}

/// The `inputs` array: one input per page, each the PNG `raster.rs` produced
/// as a data URI.
fn image_inputs(pages: &[RenderedPage]) -> Value {
    Value::Array(
        pages
            .iter()
            .map(|page| {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&page.png);
                json!({
                    "content": [{
                        "type": "image_base64",
                        "image_base64": format!("data:image/png;base64,{encoded}"),
                    }],
                })
            })
            .collect(),
    )
}

/// Read the vectors out of a response, in input order — by each item's
/// `index`, not array order, so a reordered `data` cannot file one page's
/// vector under another.
fn decode_response(payload: &Value, expected: usize) -> Result<Vec<Vec<f32>>, EmbedError> {
    let data = payload
        .get("data")
        .and_then(Value::as_array)
        .ok_or(EmbedError::Document {
            code: "invalid-response".into(),
        })?;
    if data.len() != expected {
        return Err(EmbedError::Document {
            code: "embedding-count-mismatch".into(),
        });
    }

    let mut slots: Vec<Option<Vec<f32>>> = vec![None; expected];
    for (position, item) in data.iter().enumerate() {
        let index = item
            .get("index")
            .and_then(Value::as_u64)
            .unwrap_or(position as u64) as usize;
        let slot = slots.get_mut(index).ok_or(EmbedError::Document {
            code: "embedding-index-out-of-range".into(),
        })?;
        if slot.is_some() {
            return Err(EmbedError::Document {
                code: "embedding-index-repeated".into(),
            });
        }
        *slot = Some(decode_embedding(item.get("embedding").ok_or(
            EmbedError::Document {
                code: "embedding-missing".into(),
            },
        )?)?);
    }
    slots
        .into_iter()
        .map(|slot| {
            slot.ok_or(EmbedError::Document {
                code: "embedding-missing".into(),
            })
        })
        .collect()
}

/// `output_encoding: "base64"` (f32 little-endian) -> `Vec<f32>`. A plain JSON
/// array is accepted too. Narrowing and normalising belong to
/// `embed::pack_vector` alone.
fn decode_embedding(value: &Value) -> Result<Vec<f32>, EmbedError> {
    match value {
        Value::String(encoded) => {
            let raw = base64::engine::general_purpose::STANDARD
                .decode(encoded.trim())
                .map_err(|_| EmbedError::Document {
                    code: "embedding-not-base64".into(),
                })?;
            if raw.len() % 4 != 0 {
                return Err(EmbedError::Document {
                    code: "embedding-not-f32".into(),
                });
            }
            Ok(raw
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect())
        }
        Value::Array(numbers) => numbers
            .iter()
            .map(|number| {
                number
                    .as_f64()
                    .map(|value| value as f32)
                    .ok_or(EmbedError::Document {
                        code: "embedding-not-numeric".into(),
                    })
            })
            .collect(),
        _ => Err(EmbedError::Document {
            code: "embedding-wrong-type".into(),
        }),
    }
}

// ── The seam ─────────────────────────────────────────────────────────────────

impl RequestRun for VoyageCloud {
    /// The largest request this account can get accepted: the API maximum or
    /// the tier's TPM, whichever is smaller. A request over TPM draws a 429 no
    /// pacing clears.
    fn max_tokens(&self) -> u64 {
        let tier = self.gate.tier().tpm.max(1.0) as u64;
        batch::MAX_TOKENS_PER_REQUEST.min(tier)
    }

    /// Embed a group of pages in as many requests as the current ceiling
    /// allows. Usually one; the loop covers a first request packed before a 429
    /// taught the tier.
    fn run(
        &self,
        pages: &[RenderedPage],
        on_wait: &dyn Fn(Option<Wait>),
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        let costs: Vec<u64> = pages
            .iter()
            .map(|page| batch::tokens_for(page.width, page.height))
            .collect();
        let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(pages.len());
        let mut offset = 0;

        while offset < pages.len() {
            // Re-read: the previous chunk's 429 may have shrunk it.
            let ceiling = self.max_tokens();
            let mut end = offset;
            let mut spent = 0u64;
            while end < pages.len() {
                let over = end > offset
                    && (end - offset >= batch::MAX_INPUTS_PER_REQUEST
                        || spent + costs[end] > ceiling);
                if over {
                    break;
                }
                spent += costs[end];
                end += 1;
            }

            let chunk = &pages[offset..end];
            let cost = Cost {
                tokens: spent,
                pixels: chunk.iter().map(batch::billed_pixels).sum(),
            };
            match self.send(
                image_inputs(chunk),
                INPUT_TYPE_DOCUMENT,
                chunk.len(),
                cost,
                on_wait,
            ) {
                Ok(part) => {
                    vectors.extend(part);
                    offset = end;
                }
                Err(SendFailure::Resize) => continue,
                Err(SendFailure::Embed(error)) => return Err(error),
            }
        }
        Ok(vectors)
    }
}

impl Embedder for VoyageCloud {
    fn embed(
        &self,
        pdf: &Path,
        page_count: u32,
        on_progress: &dyn Fn(Progress),
    ) -> Result<EmbedOutput, EmbedError> {
        // pdfium and the parse record's `lopdf` can disagree on page count
        // (damaged xref, lying `/Count`). `page_no` is the join key, so refuse
        // before a pixel is billed rather than file vectors under wrong pages.
        let theirs = raster::page_count(pdf)?;
        if page_count > 0 && theirs != page_count {
            return Err(EmbedError::Document {
                code: "page-count-mismatch".into(),
            });
        }
        let expected = if page_count > 0 { page_count } else { theirs };

        // Before the work, so a run that cannot fit does not half-embed.
        self.ledger.ensure_available(0)?;

        let runner: Arc<dyn RequestRun> = Arc::new(self.clone());
        let pages = batch::run_document(pdf, expected, runner, self.limits, on_progress)?;

        // Never a partial record: `EmbedOutput::new` drops out-of-range pages,
        // so drifted page numbers would otherwise leave one short.
        let output = EmbedOutput::new(pdf, expected, pages);
        if output.page_count as u32 != expected {
            return Err(EmbedError::Document {
                code: "incomplete".into(),
            });
        }
        Ok(output)
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(EmbedError::Document {
                code: "empty-query".into(),
            });
        }
        // ~4 characters a token, only to pace and reserve; the response's
        // `usage.total_tokens` settles it.
        let cost = Cost {
            tokens: (text.len() as u64 / 4) + 1,
            pixels: 0,
        };
        let inputs = json!([{ "content": [{ "type": "text", "text": text }] }]);
        let vectors = self
            .send(inputs, INPUT_TYPE_QUERY, 1, cost, &|_| {})
            .map_err(|failure| {
                match failure {
                    SendFailure::Embed(error) => error,
                    // Unreachable: `send` only resizes multi-input requests.
                    SendFailure::Resize => EmbedError::RateLimited {
                        retry_after_secs: None,
                    },
                }
            })?;
        let raw = vectors.into_iter().next().ok_or(EmbedError::Document {
            code: "embedding-missing".into(),
        })?;
        // Normalised by the same gate as stored pages.
        Ok(unpack_vector(&pack_vector(&raw)?))
    }

    /// Ready means a key (guaranteed by `new`) and a renderer: a missing
    /// libpdfium is refused once by `embed::preflight` instead of failing every
    /// file. Quota is not readiness; it surfaces as `QuotaExhausted`.
    fn health(&self) -> Health {
        Health {
            backend: BACKEND.to_string(),
            model: EMBED_MODEL.to_string(),
            dim: EMBED_DIM,
            ready: raster::available().is_ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    use super::super::ledger::{TierSource, FREE_TPM};
    use crate::embed::Engine;
    use crate::ratelimit::hold;
    use crate::test_support::{write_pdf, write_pdf_sized, FakeKeyd, FakeServer, Reply, Scratch};

    /// Never a real key; the live one stays in the keychain.
    const TEST_KEY: &str = "pa-test-only-not-a-real-key";

    /// The gates run 500x fast: real decisions, compressed clock.
    const TEST_PACE: f64 = 500.0;

    /// A live free-tier 429 body, verbatim. It arrives with no `Retry-After`.
    const LIVE_FREE_TIER_429: &str = "You have not yet added your payment method in the \
        billing page and will have reduced rate limits of 3 RPM and 10K TPM. To unlock our \
        standard rate limits, please add a payment method in the billing page...";

    fn client(fake: &FakeServer, scratch: &Scratch) -> VoyageCloud {
        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        VoyageCloud::new(&format!("{}/v1", fake.origin()), TEST_KEY)
            .unwrap()
            .with_ledger(ledger.clone())
            // A private gate so one test cannot pace another.
            .with_gate(Arc::new(RateGate::with_pace(
                ledger,
                Duration::from_secs(3_600),
                TEST_PACE,
            )))
            .with_time_scale(0.002)
    }

    /// A vector as Voyage sends it: base64 of f32 little-endian.
    fn wire_vector(seed: u32) -> String {
        let mut bytes = Vec::with_capacity(EMBED_DIM * 4);
        for index in 0..EMBED_DIM {
            let value = (((index as u32 * 31 + seed) % 97) as f32 - 48.0) / 64.0;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn ok_body(count: usize) -> Value {
        let data: Vec<Value> = (0..count)
            .map(|index| json!({ "index": index, "embedding": wire_vector(index as u32) }))
            .collect();
        json!({
            "object": "list",
            "data": data,
            "model": EMBED_MODEL,
            "usage": { "total_tokens": 3_572 * count },
        })
    }

    fn ok_response(count: usize) -> Reply {
        Reply::json(ok_body(count))
    }

    /// Landscape A4, 842 x 595 pt: over the billed-pixel cap at `RENDER_DPI`.
    fn write_a4_pdf(path: &Path, pages: usize) {
        write_pdf_sized(path, pages, 842, 595);
    }

    /// End-to-end tests skip without libpdfium (`bun run pdfium`).
    fn renderer_present() -> bool {
        let available = raster::available().is_ok();
        if !available {
            eprintln!("skipping: libpdfium not fetched (run `bun run pdfium` in app/)");
        }
        available
    }

    // ── The request on the wire ──────────────────────────────────────────────

    #[test]
    fn a_page_request_carries_the_verified_body() {
        let fake = FakeServer::start(|_| ok_response(2));
        let scratch = Scratch::new("voyage-body");
        let client = client(&fake, &scratch);

        let pages = vec![
            RenderedPage {
                page_no: 1,
                width: 100,
                height: 100,
                png: b"\x89PNGone".to_vec(),
            },
            RenderedPage {
                page_no: 2,
                width: 100,
                height: 100,
                png: b"\x89PNGtwo".to_vec(),
            },
        ];
        let vectors = client.run(&pages, &|_| {}).unwrap();
        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0].len(), EMBED_DIM);

        let hit = &fake.hits()[0];
        let body = hit.json();
        assert_eq!(body["model"], EMBED_MODEL);
        assert_eq!(body["input_type"], "document");
        assert_eq!(body["output_dimension"], EMBED_DIM);
        // Transport, not precision: `output_dtype` must not appear.
        assert_eq!(body["output_encoding"], "base64");
        assert!(body.get("output_dtype").is_none(), "{body}");

        let first = &body["inputs"][0]["content"][0];
        assert_eq!(first["type"], "image_base64");
        assert!(
            first["image_base64"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,"),
            "{first}"
        );
        assert_eq!(
            hit.header("authorization"),
            Some(format!("Bearer {TEST_KEY}").as_str())
        );
    }

    #[test]
    fn a_query_uses_the_other_side_of_the_asymmetry() {
        let fake = FakeServer::start(|_| ok_response(1));
        let scratch = Scratch::new("voyage-query");
        let client = client(&fake, &scratch);

        let vector = client.embed_query("what is a martingale").unwrap();
        assert_eq!(vector.len(), EMBED_DIM);
        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 2e-3, "{norm}");

        let body = fake.hits()[0].json();
        assert_eq!(body["input_type"], "query");
        assert_eq!(body["inputs"][0]["content"][0]["type"], "text");
    }

    // ── Decoding ─────────────────────────────────────────────────────────────

    #[test]
    fn base64_is_f32_little_endian_at_two_thousand_and_forty_eight_bytes() {
        let encoded = wire_vector(0);
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .unwrap();
        assert_eq!(raw.len(), 2_048);
        assert_eq!(raw.len(), EMBED_DIM * 4);

        let decoded = decode_embedding(&Value::String(encoded)).unwrap();
        assert_eq!(decoded.len(), EMBED_DIM);
        assert_eq!(decoded[0], -48.0 / 64.0);
        assert_eq!(decoded[1], (31.0 - 48.0) / 64.0);

        // Decoding must not normalise; that is `pack_vector`'s job.
        let norm = decoded
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        assert!(norm > 2.0, "decode must not normalise: {norm}");
    }

    #[test]
    fn a_json_float_array_still_decodes() {
        let decoded = decode_embedding(&json!([0.5, -0.25, 0.125])).unwrap();
        assert_eq!(decoded, vec![0.5, -0.25, 0.125]);
    }

    #[test]
    fn a_malformed_vector_is_this_documents_problem() {
        for value in [
            Value::String("not base64 at all !!!".into()),
            Value::String(base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3])),
            json!({ "unexpected": true }),
            json!(["not a number"]),
        ] {
            let error = decode_embedding(&value).unwrap_err();
            assert_eq!(error.kind(), "document", "{value}");
            assert!(!error.latching());
        }
    }

    #[test]
    fn vectors_are_filed_by_the_index_the_server_gave_them() {
        let payload = json!({
            "data": [
                { "index": 1, "embedding": wire_vector(11) },
                { "index": 0, "embedding": wire_vector(0) },
            ],
        });
        let vectors = decode_response(&payload, 2).unwrap();
        assert_eq!(
            vectors[0],
            decode_embedding(&json!(wire_vector(0))).unwrap()
        );
        assert_eq!(
            vectors[1],
            decode_embedding(&json!(wire_vector(11))).unwrap()
        );

        // Short, repeated and out-of-range answers are refused.
        assert!(decode_response(&payload, 3).is_err());
        assert!(decode_response(
            &json!({ "data": [
                { "index": 0, "embedding": wire_vector(0) },
                { "index": 0, "embedding": wire_vector(1) },
            ] }),
            2
        )
        .is_err());
        assert!(decode_response(
            &json!({ "data": [{ "index": 9, "embedding": wire_vector(0) }] }),
            1
        )
        .is_err());
        assert!(decode_response(&json!({ "error": "nope" }), 1).is_err());
    }

    // ── Credentials, quota, throttling ───────────────────────────────────────

    #[test]
    fn a_refused_key_is_never_retried() {
        let fake = FakeServer::start(|_| {
            Reply::status(401, json!({ "detail": "Provided API key is invalid." }))
        });
        let scratch = Scratch::new("voyage-auth");
        let client = client(&fake, &scratch);

        let error = client.embed_query("anything").unwrap_err();
        assert_eq!(error.kind(), "rejected_credentials");
        assert!(!error.retryable());
        assert!(
            error.latching(),
            "every other file would hit the same rejection"
        );
        assert_eq!(
            fake.hits().len(),
            1,
            "a rejected key cannot be retried into working"
        );
    }

    #[test]
    fn an_expired_key_says_so() {
        let fake = FakeServer::start(|_| {
            Reply::status(403, json!({ "detail": "This API key has expired." }))
        });
        let scratch = Scratch::new("voyage-expired");
        let error = client(&fake, &scratch).embed_query("x").unwrap_err();
        assert!(
            matches!(error, EmbedError::RejectedCredentials { expired: true, .. }),
            "{error:?}"
        );
    }

    #[test]
    fn money_latches_the_ledger_but_pace_does_not() {
        let fake = FakeServer::start(|_| {
            Reply::status(
                403,
                json!({ "detail": "Your account has run out of credit." }),
            )
        });
        let scratch = Scratch::new("voyage-credit");
        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        let client = VoyageCloud::new(&format!("{}/v1", fake.origin()), TEST_KEY)
            .unwrap()
            .with_ledger(ledger.clone())
            .with_gate(Arc::new(RateGate::with_pace(
                ledger.clone(),
                Duration::from_secs(3_600),
                TEST_PACE,
            )))
            .with_time_scale(0.002);

        let error = client.embed_query("x").unwrap_err();
        assert_eq!(error.kind(), "quota_exhausted");
        assert!(error.latching(), "every file draws on the same allowance");
        assert!(
            error.retryable(),
            "and it repairs itself when the account is topped up"
        );
        assert!(ledger.snapshot().latched());
        assert!(matches!(
            ledger.ensure_available(0),
            Err(EmbedError::QuotaExhausted)
        ));
    }

    #[test]
    fn a_429_is_waited_out_and_its_stated_limit_is_learned() {
        let fake = FakeServer::start(|hit| {
            if hit.index == 0 {
                Reply::status(
                    429,
                    json!({ "detail": "Rate limit exceeded: 3 requests per minute (RPM) and \
                                        10000 tokens per minute (TPM) for this account." }),
                )
                .with_header("Retry-After", "2")
            } else {
                ok_response(1)
            }
        });
        let scratch = Scratch::new("voyage-throttle");
        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        let gate = Arc::new(RateGate::with_pace(
            ledger.clone(),
            Duration::from_secs(3_600),
            TEST_PACE,
        ));
        let client = VoyageCloud::new(&format!("{}/v1", fake.origin()), TEST_KEY)
            .unwrap()
            .with_ledger(ledger.clone())
            .with_gate(gate.clone())
            .with_time_scale(0.002);

        client.embed_query("hello").unwrap();
        assert_eq!(fake.hits().len(), 2);

        let tier = gate.tier();
        assert_eq!(tier.source, TierSource::Stated);
        assert_eq!(tier.tpm, FREE_TPM);
        // Persisted for the next run.
        assert_eq!(ledger.tier().tpm, FREE_TPM);
    }

    #[test]
    fn a_429_reports_its_wait_and_clears_it_once_the_request_is_admitted() {
        let fake = FakeServer::start(|hit| {
            if hit.index == 0 {
                Reply::status(429, json!({ "detail": LIVE_FREE_TIER_429 }))
                    .with_header("Retry-After", "30")
            } else {
                ok_response(1)
            }
        });
        let scratch = Scratch::new("voyage-wait-notice");
        let client = client(&fake, &scratch);
        let pages = vec![RenderedPage {
            page_no: 1,
            width: 100,
            height: 100,
            png: b"\x89PNG".to_vec(),
        }];

        let before = Wait::after(Duration::ZERO, Limiter::Throttled).until_ms;
        let told = Mutex::new(Vec::new());
        client.run(&pages, &|wait| hold(&told).push(wait)).unwrap();
        let told = hold(&told).clone();

        assert_eq!(told.len(), 2, "one wait, then its end: {told:?}");
        let wait = told[0].expect("the 429's wait");
        assert_eq!(wait.limiter, Limiter::Throttled);
        assert!(wait.until_ms >= before, "{wait:?}");
        assert_eq!(told[1], None);
        assert_eq!(Limiter::Throttled.describe(), "rate-limited by Voyage");
        assert_eq!(
            Limiter::Requests { per_minute: 3 }.describe(),
            "pacing to Voyage's 3 requests/min limit"
        );
        assert_eq!(
            Limiter::Tokens { per_minute: 10_000 }.describe(),
            "pacing to Voyage's 10K tokens/min limit"
        );
    }

    #[test]
    fn an_endless_429_eventually_becomes_a_retryable_error_rather_than_a_parked_thread() {
        let fake = FakeServer::start(|_| {
            Reply::status(429, json!({ "detail": "slow down" })).with_header("Retry-After", "1")
        });
        let scratch = Scratch::new("voyage-endless");
        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        let client = VoyageCloud::new(&format!("{}/v1", fake.origin()), TEST_KEY)
            .unwrap()
            .with_ledger(ledger.clone())
            .with_gate(Arc::new(RateGate::with_pace(
                ledger,
                Duration::from_secs(3_600),
                TEST_PACE,
            )))
            .with_time_scale(0.000_02);

        let error = client.embed_query("x").unwrap_err();
        assert_eq!(error.kind(), "rate_limited");
        assert!(error.retryable());
        assert!(!error.latching());
    }

    #[test]
    fn a_server_fault_is_retried_and_then_reported_as_transport() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let seen = attempts.clone();
        let fake = FakeServer::start(move |_| {
            seen.fetch_add(1, Ordering::SeqCst);
            Reply::status(503, json!({ "detail": "upstream" }))
        });
        let scratch = Scratch::new("voyage-fault");
        let error = client(&fake, &scratch).embed_query("x").unwrap_err();

        assert_eq!(error.kind(), "offline");
        assert!(error.retryable());
        assert_eq!(attempts.load(Ordering::SeqCst), ATTEMPTS as usize);
    }

    // ── Nothing leaks ────────────────────────────────────────────────────────

    #[test]
    fn an_error_never_carries_the_key_a_url_or_the_server_body() {
        let fake = FakeServer::start(|_| {
            Reply::status(
                400,
                json!({
                    "detail": format!(
                        "Request with key {TEST_KEY} to https://signed.example/upload?sig=SECRET \
                         failed; inputs were data:image/png;base64,iVBORw0KGgo"
                    ),
                }),
            )
        });
        let scratch = Scratch::new("voyage-leak");
        let error = client(&fake, &scratch).embed_query("x").unwrap_err();

        for rendering in [error.to_string(), format!("{error:?}")] {
            assert!(!rendering.contains(TEST_KEY), "{rendering}");
            assert!(!rendering.contains("SECRET"), "{rendering}");
            assert!(!rendering.contains("signed.example"), "{rendering}");
            assert!(!rendering.contains("://"), "{rendering}");
            assert!(!rendering.contains("base64"), "{rendering}");
        }
        assert!(format!("{error:?}").contains("400"), "{error:?}");
    }

    // ── A whole document ─────────────────────────────────────────────────────

    #[test]
    fn a_document_embeds_every_page_and_reports_real_progress() {
        if !renderer_present() {
            return;
        }
        let fake = FakeServer::start(|hit| {
            let inputs = hit.json()["inputs"].as_array().map(Vec::len).unwrap_or(0);
            ok_response(inputs)
        });
        let scratch = Scratch::new("voyage-document");
        let pdf = scratch.join("deck.pdf");
        write_pdf(&pdf, 5);

        let client = client(&fake, &scratch).with_limits(Limits {
            max_inputs: 2,
            max_tokens: batch::MAX_TOKENS_PER_REQUEST,
            in_flight: 2,
        });

        let progress = Mutex::new(Vec::new());
        let output = client
            .embed(&pdf, 5, &|update| hold(&progress).push(update.pages_done))
            .unwrap();

        assert_eq!(output.page_count, 5);
        assert_eq!(
            output.pages.iter().map(|p| p.page_no).collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        assert_eq!(output.model, EMBED_MODEL);
        assert_eq!(output.dim, EMBED_DIM);
        assert_eq!(fake.hits().len(), 3, "5 pages at 2 per request");

        let seen = hold(&progress).clone();
        assert_eq!(seen.last().copied(), Some(5));
        assert!(seen.windows(2).all(|pair| pair[0] <= pair[1]), "{seen:?}");
    }

    #[test]
    fn a_document_that_could_not_embed_every_page_writes_nothing() {
        if !renderer_present() {
            return;
        }
        // The second request fails; the first one's vectors must still not
        // become a record.
        let fake = FakeServer::start(|hit| {
            let inputs = hit.json()["inputs"].as_array().map(Vec::len).unwrap_or(0);
            if hit.index == 1 {
                Reply::status(400, json!({ "detail": "no" }))
            } else {
                ok_response(inputs)
            }
        });
        let scratch = Scratch::new("voyage-partial");
        let pdf = scratch.join("deck.pdf");
        write_pdf(&pdf, 4);

        let client = client(&fake, &scratch).with_limits(Limits {
            max_inputs: 1,
            max_tokens: batch::MAX_TOKENS_PER_REQUEST,
            in_flight: 1,
        });
        let error = client.embed(&pdf, 4, &|_| {}).unwrap_err();
        assert_eq!(error.kind(), "document");
        assert!(
            !error.latching(),
            "one bad document must not condemn the run"
        );
        assert!(!crate::embed::emb_path(&pdf).exists());
    }

    #[test]
    fn a_free_tier_account_shrinks_its_requests_instead_of_retrying_forever() {
        if !renderer_present() {
            return;
        }
        // Like the live server: anything over the account's TPM is refused
        // outright, with no `Retry-After`.
        let fake = FakeServer::start(|hit| {
            let inputs = hit.json()["inputs"].as_array().map(Vec::len).unwrap_or(0);
            let tokens = inputs as u64 * batch::tokens_for(2339, 1653);
            if tokens > 10_000 {
                Reply::status(429, json!({ "detail": LIVE_FREE_TIER_429 }))
            } else {
                ok_response(inputs)
            }
        });
        let scratch = Scratch::new("voyage-livelock");
        let pdf = scratch.join("deck.pdf");
        write_a4_pdf(&pdf, 4);

        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        let gate = Arc::new(RateGate::with_pace(
            ledger.clone(),
            Duration::from_secs(3_600),
            TEST_PACE,
        ));
        let client = VoyageCloud::new(&format!("{}/v1", fake.origin()), TEST_KEY)
            .unwrap()
            .with_ledger(ledger.clone())
            .with_gate(gate.clone())
            .with_time_scale(0.002);

        // Packed optimistically into one over-TPM request.
        assert_eq!(client.max_tokens(), batch::MAX_TOKENS_PER_REQUEST);

        let output = client
            .embed(&pdf, 4, &|_| {})
            .expect("the run must make progress");
        assert_eq!(output.page_count, 4);
        assert_eq!(
            output.pages.iter().map(|p| p.page_no).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );

        assert_eq!(gate.tier().tpm, 10_000.0);
        assert_eq!(gate.tier().rpm, 3.0);
        assert_eq!(client.max_tokens(), 10_000);

        // One refusal, then two accepted halves.
        let sizes: Vec<usize> = fake
            .hits()
            .iter()
            .map(|hit| hit.json()["inputs"].as_array().map(Vec::len).unwrap_or(0))
            .collect();
        assert_eq!(sizes, vec![4, 2, 2], "{sizes:?}");
    }

    #[test]
    fn a_single_page_is_never_too_big_to_shrink_to() {
        // Repacking terminates: the billing cap keeps any page under the
        // slowest tier's TPM.
        assert!(batch::tokens_for(u32::MAX, u32::MAX) < 10_000);
    }

    #[test]
    fn a_page_count_the_two_counters_disagree_on_is_a_document_error() {
        if !renderer_present() {
            return;
        }
        let fake = FakeServer::start(|_| ok_response(1));
        let scratch = Scratch::new("voyage-count");
        let pdf = scratch.join("deck.pdf");
        write_pdf(&pdf, 3);

        // The parse record says four pages; pdfium finds three.
        let error = client(&fake, &scratch).embed(&pdf, 4, &|_| {}).unwrap_err();
        assert_eq!(error.kind(), "document");
        assert!(
            format!("{error:?}").contains("page-count-mismatch"),
            "{error:?}"
        );
        assert!(fake.hits().is_empty());
    }

    #[test]
    fn a_latching_failure_stops_the_document_rather_than_embedding_the_rest_of_it() {
        if !renderer_present() {
            return;
        }
        let fake = FakeServer::start(|_| {
            Reply::status(401, json!({ "detail": "Provided API key is invalid." }))
        });
        let scratch = Scratch::new("voyage-latch");
        let pdf = scratch.join("deck.pdf");
        write_pdf(&pdf, 6);

        let client = client(&fake, &scratch).with_limits(Limits {
            max_inputs: 1,
            max_tokens: batch::MAX_TOKENS_PER_REQUEST,
            in_flight: 1,
        });
        let error = client.embed(&pdf, 6, &|_| {}).unwrap_err();
        assert!(error.latching(), "{error:?}");
        assert!(fake.hits().len() < 6, "{} requests", fake.hits().len());
    }

    #[test]
    fn health_refuses_the_run_when_the_renderer_is_missing() {
        let fake = FakeServer::start(|_| ok_response(1));
        let scratch = Scratch::new("voyage-health");
        let health = client(&fake, &scratch).health();
        assert_eq!(health.model, EMBED_MODEL);
        assert_eq!(health.dim, EMBED_DIM);
        assert_eq!(health.ready, raster::available().is_ok());
        if health.ready {
            health.check().unwrap();
        } else {
            assert_eq!(health.check().unwrap_err().kind(), "not_ready");
        }
    }

    #[test]
    fn a_missing_key_is_refused_before_a_client_exists() {
        assert!(matches!(
            VoyageCloud::new("https://example.invalid/v1", "   "),
            Err(EmbedError::MissingCredentials)
        ));
    }

    // ── Through oculus-keyd ──────────────────────────────────────────────────

    fn cloud() -> EmbedConfig {
        EmbedConfig {
            engine: Engine::Cloud,
            base_url: CLOUD_BASE_URL.to_string(),
            credentials: CredentialSource::Keychain,
        }
    }

    /// A keyd holding a key that answers the `n`th `forward` with `answer(n)`.
    fn keyd_with<F>(scratch: &Scratch, answer: F) -> FakeKeyd
    where
        F: Fn(usize) -> (Value, Vec<u8>) + Send + 'static,
    {
        let forwards = AtomicUsize::new(0);
        FakeKeyd::start(scratch, move |req, _| match req["op"].as_str().unwrap() {
            "has" => (json!({ "has": true }), vec![]),
            "forward" => answer(forwards.fetch_add(1, Ordering::SeqCst)),
            op => (
                json!({ "error": "request", "detail": format!("unexpected {op}") }),
                vec![],
            ),
        })
    }

    fn forwarded(status: u16, headers: Value, body: &Value) -> (Value, Vec<u8>) {
        let body = body.to_string().into_bytes();
        (
            json!({ "status": status, "headers": headers, "body_len": body.len() }),
            body,
        )
    }

    fn keyd_client(scratch: &Scratch) -> VoyageCloud {
        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        let Ok(client) = VoyageCloud::with_config_in(&cloud(), scratch, || {
            panic!("the keychain must not be read while keyd is installed")
        }) else {
            panic!("no client through keyd");
        };
        client
            .with_ledger(ledger.clone())
            .with_gate(Arc::new(RateGate::with_pace(
                ledger,
                Duration::from_secs(3_600),
                TEST_PACE,
            )))
            .with_time_scale(0.002)
    }

    fn config_error(config: &EmbedConfig, scratch: &Scratch) -> EmbedError {
        match VoyageCloud::with_config_in(config, scratch, || Ok(Some(TEST_KEY.to_string()))) {
            Ok(_) => panic!("expected an error"),
            Err(error) => error,
        }
    }

    #[test]
    fn the_keyd_path_is_the_cloud_roots_path() {
        assert!(CLOUD_BASE_URL.ends_with(CLOUD_PATH));
        assert!(!CLOUD_BASE_URL[..CLOUD_BASE_URL.len() - CLOUD_PATH.len()].contains("/v"));
    }

    #[test]
    fn through_keyd_a_query_is_forwarded_without_the_key() {
        let scratch = Scratch::new("vk-ok");
        let keyd = keyd_with(&scratch, |_| forwarded(200, json!([]), &ok_body(1)));
        let vector = keyd_client(&scratch)
            .embed_query("what is a martingale")
            .unwrap();
        assert_eq!(vector.len(), EMBED_DIM);

        assert_eq!(keyd.ops(), ["has", "forward"]);
        let (header, body) = &keyd.requests()[1];
        assert_eq!(header["secret"], "voyage");
        assert_eq!(header["method"], "POST");
        assert_eq!(header["path"], "/v1/multimodalembeddings");
        assert_eq!(
            header["headers"],
            json!([
                ["Content-Type", "application/json"],
                ["Accept", "application/json"]
            ])
        );
        assert!(
            !header.to_string().to_lowercase().contains("authorization"),
            "{header}"
        );
        let sent: Value = serde_json::from_slice(body).unwrap();
        assert_eq!(sent["input_type"], "query");
        assert_eq!(sent["output_encoding"], "base64");
    }

    #[test]
    fn through_keyd_a_429_is_waited_out_and_its_stated_limit_is_learned() {
        let scratch = Scratch::new("vk-429");
        let keyd = keyd_with(&scratch, |n| {
            if n == 0 {
                forwarded(
                    429,
                    json!([["retry-after", "2"]]),
                    &json!({ "detail": "Rate limit exceeded: 3 requests per minute (RPM) and \
                                        10000 tokens per minute (TPM) for this account." }),
                )
            } else {
                forwarded(200, json!([]), &ok_body(1))
            }
        });
        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        let gate = Arc::new(RateGate::with_pace(
            ledger.clone(),
            Duration::from_secs(3_600),
            TEST_PACE,
        ));
        let client = keyd_client(&scratch)
            .with_ledger(ledger)
            .with_gate(gate.clone());

        client.embed_query("hello").unwrap();
        assert_eq!(keyd.ops(), ["has", "forward", "forward"]);
        assert_eq!(gate.tier().source, TierSource::Stated);
        assert_eq!(gate.tier().tpm, FREE_TPM);
    }

    #[test]
    fn through_keyd_a_refused_key_and_spent_credit_read_as_they_do_direct() {
        let scratch = Scratch::new("vk-401");
        let keyd = keyd_with(&scratch, |_| {
            forwarded(
                401,
                json!([]),
                &json!({ "detail": "This API key has expired." }),
            )
        });
        let error = keyd_client(&scratch).embed_query("x").unwrap_err();
        assert!(
            matches!(error, EmbedError::RejectedCredentials { expired: true, .. }),
            "{error:?}"
        );
        assert_eq!(
            keyd.ops(),
            ["has", "forward"],
            "a rejected key is not retried"
        );

        let scratch = Scratch::new("vk-402");
        let _keyd = keyd_with(&scratch, |_| {
            forwarded(
                403,
                json!([]),
                &json!({ "detail": "Your account has run out of credit." }),
            )
        });
        assert_eq!(
            keyd_client(&scratch).embed_query("x").unwrap_err().kind(),
            "quota_exhausted"
        );
    }

    #[test]
    fn keyd_without_a_key_is_missing_before_and_during_a_request() {
        let scratch = Scratch::new("vk-none");
        let _keyd = FakeKeyd::start(&scratch, |_, _| (json!({ "has": false }), vec![]));
        assert!(matches!(
            config_error(&cloud(), &scratch),
            EmbedError::MissingCredentials
        ));

        // Deleted between the check and the request.
        let scratch = Scratch::new("vk-gone");
        let _keyd = keyd_with(&scratch, |_| {
            (
                json!({ "error": "missing", "detail": "no voyage key is stored" }),
                vec![],
            )
        });
        let error = keyd_client(&scratch).embed_query("x").unwrap_err();
        assert_eq!(error.kind(), "missing_credentials");
    }

    #[test]
    fn keyd_reaching_no_origin_backs_off_then_reads_as_offline() {
        let scratch = Scratch::new("vk-upstream");
        let keyd = keyd_with(&scratch, |_| {
            (
                json!({ "error": "upstream", "detail": "Dns Failed: no such host" }),
                vec![],
            )
        });
        let error = keyd_client(&scratch).embed_query("x").unwrap_err();
        assert_eq!(error.kind(), "offline");
        assert!(error.retryable());
        assert!(error.to_string().contains("Dns Failed"), "{error}");
        assert_eq!(keyd.ops().len(), 1 + ATTEMPTS as usize);
    }

    #[test]
    fn keyds_own_refusals_surface_by_kind() {
        for (wire, kind) in [
            ("keychain", "unreadable_credentials"),
            ("caller", "credential_broker"),
            ("vault", "credential_broker"),
        ] {
            let scratch = Scratch::new("vk-refused");
            let _keyd = FakeKeyd::start(&scratch, move |_, _| {
                (json!({ "error": wire, "detail": "refused" }), vec![])
            });
            let error = config_error(&cloud(), &scratch);
            assert_eq!(error.kind(), kind, "{wire}");
            assert!(error.latching(), "{wire}");
        }
    }

    #[test]
    fn without_keyd_or_off_voyages_root_the_key_is_read_directly() {
        // No socket: today's path, with the keychain's key.
        let scratch = Scratch::new("vk-absent");
        let Ok(client) =
            VoyageCloud::with_config_in(&cloud(), &scratch, || Ok(Some(TEST_KEY.to_string())))
        else {
            panic!("a direct client");
        };
        assert!(matches!(&client.auth, Auth::Direct(key) if key.as_str() == TEST_KEY));
        assert!(matches!(
            config_error_none(&scratch),
            EmbedError::MissingCredentials
        ));

        // A hand-edited `engineUrl` never goes through keyd, even when it is there.
        let scratch = Scratch::new("vk-elsewhere");
        let keyd = keyd_with(&scratch, |_| forwarded(500, json!([]), &json!({})));
        let fake = FakeServer::start(|_| ok_response(1));
        let config = EmbedConfig {
            base_url: format!("{}/v1", fake.origin()),
            ..cloud()
        };
        let Ok(client) =
            VoyageCloud::with_config_in(&config, &scratch, || Ok(Some(TEST_KEY.to_string())))
        else {
            panic!("a direct client");
        };
        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        client
            .with_ledger(ledger.clone())
            .with_gate(Arc::new(RateGate::with_pace(
                ledger,
                Duration::from_secs(3_600),
                TEST_PACE,
            )))
            .embed_query("x")
            .unwrap();
        assert!(keyd.requests().is_empty());
        assert_eq!(
            fake.hits()[0].header("authorization"),
            Some(format!("Bearer {TEST_KEY}").as_str())
        );
    }

    /// No keyd and no key in the keychain.
    fn config_error_none(scratch: &Scratch) -> EmbedError {
        match VoyageCloud::with_config_in(&cloud(), scratch, || Ok(None)) {
            Ok(_) => panic!("expected an error"),
            Err(error) => error,
        }
    }

    /// The one test that talks to Voyage, off unless `OCULUS_VOYAGE_LIVE=1`.
    /// Embeds three small pages with the keychain's key. Keep it small: on the
    /// free tier a 429 is the expected answer and the run retries through it.
    #[test]
    fn a_real_call_against_voyage() {
        if std::env::var_os("OCULUS_VOYAGE_LIVE").is_none() {
            eprintln!("skipping: set OCULUS_VOYAGE_LIVE=1 to spend real quota");
            return;
        }
        if !renderer_present() {
            return;
        }
        let client = match VoyageCloud::from_config() {
            Ok(client) => client,
            Err(error) => {
                eprintln!("skipping: {error}");
                return;
            }
        };
        client.health().check().unwrap();

        let scratch = Scratch::new("voyage-live");
        let pdf = scratch.join("live.pdf");
        write_pdf(&pdf, 3);

        let output = client.embed(&pdf, 3, &|update| {
            eprintln!("live: {}/{}", update.pages_done, update.total_pages);
        });
        let output = output.expect("live embed");

        assert_eq!(output.page_count, 3);
        assert_eq!(output.dim, EMBED_DIM);
        assert_eq!(output.model, EMBED_MODEL);
        for page in &output.pages {
            let vector = crate::embed::decode_vector(&page.vector).unwrap();
            assert_eq!(vector.len(), EMBED_DIM);
            let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
            assert!(
                (norm - 1.0).abs() < 2e-3,
                "page {}: ‖v‖ = {norm}",
                page.page_no
            );
        }
    }
}

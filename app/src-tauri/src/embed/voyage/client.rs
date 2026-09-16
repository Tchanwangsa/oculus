//! The Voyage multimodal protocol, and the `Embedder` the app indexes through.
//!
//! The conversation is far simpler than MinerU's — one `POST`, one answer, no
//! signed upload and no polling — so almost all of this file is about the two
//! things that are *harder* here: the transport encoding, and the rate limit.
//!
//! **Two parameters are easy to confuse, and both were checked against the live
//! API on 2026-09-17.** `output_dtype` is *precision* and names
//! `['binary', 'float', 'int8', …]` — there is no `float16` and it does not
//! accept `base64`. `output_encoding` is *transport*, and `"base64"` there does
//! work: it returns a base64 NumPy array, measured at **2048 bytes for 512
//! dims, i.e. f32 little-endian**. Using it avoids parsing a 512-element JSON
//! float array per page; the f16 narrowing, the truncation and the
//! re-normalisation all happen once, in the seam's `pack_vector`, and never
//! here.
//!
//! Three rules run through the rest, and two of them are the parser's:
//!
//! * **Errors carry a code, never the server's text.** A Voyage error body can
//!   echo the request, which for this API means an echo of a page image — and,
//!   on a 429, the account's billing state. Every `EmbedError` built here is a
//!   short code or a local transport message.
//! * **Failures are scoped.** A page that will not rasterise, a vector of the
//!   wrong width, a malformed answer: that document only. Credentials and quota
//!   condemn the run, and `EmbedError::latching` is exactly that question.
//! * **A 429 is not a failure.** It is what a working run looks like most of the
//!   time on an account with no card, and it is also the only place the
//!   account's real limits are ever stated. It is honoured, learned from, and
//!   retried without consuming an attempt.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{json, Value};

use crate::embed::raster::{self, RenderedPage};
use crate::embed::{
    embed_config, pack_vector, unpack_vector, EmbedConfig, EmbedError, EmbedOutput, Embedder,
    Health, Progress, EMBED_DIM, EMBED_MODEL,
};

use super::batch::{self, Limits, RequestRun};
use super::ledger::{is_about_credit, RateGate, UsageLedger};

/// The `backend` this client reports, and what `Progress` is stamped with.
pub const BACKEND: &str = "voyage-cloud";

/// The endpoint. Images only go here — `/embeddings` is the text-only model's
/// and does not accept an image input at all.
const EMBED_PATH: &str = "/multimodalembeddings";

/// **The asymmetry.** The model is trained with different sides for stored
/// documents and for questions asked of them, and using one for both is not an
/// error anything can detect — it just quietly ranks worse. The seam records
/// the convention as `QUERY_INSTRUCTION`; these are what actually go on the
/// wire.
const INPUT_TYPE_DOCUMENT: &str = "document";
const INPUT_TYPE_QUERY: &str = "query";

/// Attempts per request. A 429 deliberately does not consume one.
const ATTEMPTS: u32 = 4;
const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(10);
/// Generous: a 46-page request is ~20 MB of base64 going up and the model has
/// to look at every page of it.
const API_TIMEOUT: Duration = Duration::from_secs(300);

/// How long one request may spend being throttled before it gives up and
/// becomes a `RateLimited` the caller can retry later.
///
/// A 429 is routine and loops here without consuming an attempt, which on the
/// free programme is the correct behaviour and can legitimately mean minutes of
/// waiting. It must still be *bounded*: an account whose limit has been set to
/// zero, or a proxy answering 429 to everything, would otherwise park an
/// indexing thread for the lifetime of the app. This is the MinerU client's
/// `POLL_DEADLINE` playing the same role.
const THROTTLE_DEADLINE: Duration = Duration::from_secs(30 * 60);

/// What one request is about to cost, reserved before it is sent.
#[derive(Debug, Clone, Copy, Default)]
struct Cost {
    tokens: u64,
    pixels: u64,
}

/// Why `send` came back without vectors.
///
/// `Resize` is not a failure at all — it is the answer to a 429 that taught us
/// the account is smaller than the request we just built. **An over-ceiling
/// request can never be accepted at any pace**, so retrying it is a livelock
/// rather than a throttle; it is handed back to be repacked smaller. Nothing
/// outside this module sees it.
enum SendFailure {
    Embed(EmbedError),
    Resize,
}

impl From<EmbedError> for SendFailure {
    fn from(error: EmbedError) -> Self {
        SendFailure::Embed(error)
    }
}

#[derive(Clone)]
pub struct VoyageCloud {
    base_url: Arc<String>,
    key: Arc<String>,
    ledger: Arc<UsageLedger>,
    gate: Arc<RateGate>,
    limits: Limits,
    /// Every wait in this client is multiplied by this. Production is 1.0;
    /// tests shrink it so a retry ladder costs milliseconds instead of
    /// seconds. Straight out of `MinerUCloud`.
    time_scale: f64,
}

impl VoyageCloud {
    /// The client the app uses: engine and API root from the settings row, key
    /// from the keychain.
    pub fn from_config() -> Result<Self, EmbedError> {
        Self::with_config(&embed_config())
    }

    pub fn with_config(config: &EmbedConfig) -> Result<Self, EmbedError> {
        let key = config.credentials.key().unwrap_or_default();
        Self::new(&config.base_url, &key)
    }

    /// `base_url` is always passed in — nothing here knows Voyage's address,
    /// which is what lets the tests point the whole protocol at a local server.
    pub fn new(base_url: &str, key: &str) -> Result<Self, EmbedError> {
        let key = key.trim();
        if key.is_empty() {
            return Err(EmbedError::MissingCredentials);
        }
        Ok(Self {
            base_url: Arc::new(base_url.trim_end_matches('/').to_string()),
            key: Arc::new(key.to_string()),
            ledger: UsageLedger::shared(),
            gate: RateGate::shared(),
            limits: Limits::default(),
            time_scale: 1.0,
        })
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

    fn nap(&self, duration: Duration) {
        let scaled = duration.mul_f64(self.time_scale);
        if !scaled.is_zero() {
            std::thread::sleep(scaled);
        }
    }

    // ── One request ──────────────────────────────────────────────────────────

    /// Send one embedding request and hand back one vector per input.
    ///
    /// The whole rate-limit story lives here: reserve, wait for the gate, send,
    /// and on a 429 learn the tier and go round again without spending an
    /// attempt.
    fn send(
        &self,
        inputs: Value,
        input_type: &str,
        expected: usize,
        cost: Cost,
    ) -> Result<Vec<Vec<f32>>, SendFailure> {
        let url = format!("{}{}", self.base_url, EMBED_PATH);
        let body = json!({
            "model": EMBED_MODEL,
            "inputs": inputs,
            "input_type": input_type,
            // Matryoshka: the model nests a 512-wide vector inside its full
            // output, so this is the stored blob width and not a truncation we
            // are doing ourselves. See `embed::EMBED_DIM`.
            "output_dimension": EMBED_DIM,
            // Transport, not precision. See the module header.
            "output_encoding": "base64",
        });
        let encoded = serde_json::to_vec(&body)
            .map_err(|e| SendFailure::Embed(EmbedError::Io(format!("encode request: {e}"))))?;

        // Reserved before anything is sent, and never given back — a request
        // that dies in flight may well have been billed, and there is no
        // endpoint that says. See `ledger`.
        self.ledger.record(1, cost.tokens, cost.pixels)?;

        let mut attempt = 0;
        let mut delay = FIRST_RETRY;
        let throttled_until = Instant::now() + THROTTLE_DEADLINE.mul_f64(self.time_scale);
        // Kept so the deadline's error can say how long the last wait was —
        // a 30-minute 429 is worth reporting honestly rather than as "failed".
        #[allow(unused_assignments)]
        let mut last_retry_after: Option<u64> = None;

        while attempt < ATTEMPTS {
            self.gate.admit(cost.tokens);
            let sent = ureq::post(&url)
                .timeout(API_TIMEOUT)
                .set("Authorization", &format!("Bearer {}", self.key))
                .set("Content-Type", "application/json")
                .set("Accept", "application/json")
                .send_bytes(&encoded);

            let response = match sent {
                Ok(response) => response,
                Err(ureq::Error::Status(status, response)) => {
                    let retry_after = response
                        .header("Retry-After")
                        .and_then(|value| value.trim().parse::<f64>().ok());
                    let text = response.into_string().unwrap_or_default();

                    if status == 429 {
                        // Routine, not fatal. The body is the one place the
                        // account's real limits are ever stated, so it is read
                        // for numbers and then dropped.
                        let wait = self.gate.throttled(retry_after, &text);
                        last_retry_after = Some(wait.as_secs());

                        // **The livelock guard.** If what we just learned puts
                        // this request over the account's per-minute ceiling,
                        // no wait will ever make it acceptable — measured live,
                        // ~14,284 tokens is refused outright on a 10K TPM
                        // account. Hand it back to be repacked smaller instead
                        // of resending the identical body once a minute for
                        // ever. A single input cannot be split, so it takes the
                        // ordinary wait-and-retry path.
                        if expected > 1 && cost.tokens > self.max_tokens() {
                            return Err(SendFailure::Resize);
                        }
                        if Instant::now() >= throttled_until {
                            return Err(EmbedError::RateLimited {
                                retry_after_secs: last_retry_after,
                            }
                            .into());
                        }
                        self.nap(wait);
                        // Deliberately does not consume an attempt.
                        continue;
                    }
                    match self.status_error(status, &text, &mut attempt, &mut delay) {
                        Some(error) => return Err(error.into()),
                        // A 5xx with attempts left: already backed off.
                        None => continue,
                    }
                }
                Err(ureq::Error::Transport(transport)) => {
                    if attempt + 1 < ATTEMPTS {
                        self.backoff(&mut attempt, &mut delay);
                        continue;
                    }
                    return Err(EmbedError::Offline(transport_detail(&transport)).into());
                }
            };

            let text = response.into_string().unwrap_or_default();
            let payload = match serde_json::from_str::<Value>(&text) {
                Ok(payload) => payload,
                Err(_) if attempt + 1 < ATTEMPTS => {
                    self.backoff(&mut attempt, &mut delay);
                    continue;
                }
                Err(_) => return Err(EmbedError::Offline("unreadable response".into()).into()),
            };

            // Voyage reports what it actually billed. That is the only
            // authority there is, so the estimate is corrected up from it —
            // and a calm response is also what raises the throttle's ceiling.
            let billed = payload
                .get("usage")
                .and_then(|usage| usage.get("total_tokens"))
                .and_then(Value::as_u64);
            self.gate.succeeded(cost.tokens, billed);

            return decode_response(&payload, expected).map_err(SendFailure::Embed);
        }
        Err(EmbedError::Offline("request failed".into()).into())
    }

    /// Turn a non-429 error status into the seam's vocabulary.
    ///
    /// `None` means "backed off, go round again" — the 5xx-with-attempts-left
    /// case. Everything else is a decision.
    ///
    /// **`body` is read and never kept.** It decides between money and a bad
    /// key, and then it is dropped: nothing it contains reaches an error, a log
    /// or the UI.
    fn status_error(
        &self,
        status: u16,
        body: &str,
        attempt: &mut u32,
        delay: &mut Duration,
    ) -> Option<EmbedError> {
        // Money, not pace. The server's own answer outranks the local guess,
        // exactly as MinerU's `-60018` does.
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
            if *attempt + 1 < ATTEMPTS {
                self.backoff(attempt, delay);
                return None;
            }
            // A server fault is not this document's fault, and `Document`
            // would mark the file permanently failed for something transient.
            return Some(EmbedError::Offline(format!("http {status}")));
        }
        // Only the status. The body beside it may quote the request back.
        Some(EmbedError::Document { code: format!("http-{status}") })
    }

    /// One step of the shared retry ladder: 1s, doubling to a 10s ceiling.
    fn backoff(&self, attempt: &mut u32, delay: &mut Duration) {
        *attempt += 1;
        self.nap(*delay);
        *delay = (*delay * 2).min(MAX_RETRY);
    }
}

/// Build the `inputs` array for a batch of rendered pages.
///
/// One input per page, one image per input. The data-URI prefix is what the
/// API expects on `image_base64`; the bytes are the PNG `raster.rs` produced,
/// so nothing in between has to decode an image.
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

/// Read the vectors out of a response, in the order the inputs went in.
///
/// Voyage returns each embedding with its own `index`, and this trusts that
/// index rather than the array order: a reordered `data` array would otherwise
/// file page 7's vector under page 3, which is invisible, permanent and
/// exactly the failure the `page_no` join key cannot survive.
fn decode_response(payload: &Value, expected: usize) -> Result<Vec<Vec<f32>>, EmbedError> {
    let data = payload
        .get("data")
        .and_then(Value::as_array)
        .ok_or(EmbedError::Document { code: "invalid-response".into() })?;
    if data.len() != expected {
        return Err(EmbedError::Document { code: "embedding-count-mismatch".into() });
    }

    let mut slots: Vec<Option<Vec<f32>>> = vec![None; expected];
    for (position, item) in data.iter().enumerate() {
        let index = item.get("index").and_then(Value::as_u64).unwrap_or(position as u64) as usize;
        let slot = slots
            .get_mut(index)
            .ok_or(EmbedError::Document { code: "embedding-index-out-of-range".into() })?;
        if slot.is_some() {
            return Err(EmbedError::Document { code: "embedding-index-repeated".into() });
        }
        *slot = Some(decode_embedding(
            item.get("embedding").ok_or(EmbedError::Document {
                code: "embedding-missing".into(),
            })?,
        )?);
    }
    slots
        .into_iter()
        .map(|slot| slot.ok_or(EmbedError::Document { code: "embedding-missing".into() }))
        .collect()
}

/// `output_encoding: "base64"` -> `&[f32]`.
///
/// The payload is a base64 NumPy buffer: **f32 little-endian**, 2048 bytes for
/// 512 dims, measured live. The JSON-array form is accepted too, so a proxy or
/// a future default that drops the encoding parameter degrades to slow rather
/// than broken.
///
/// Nothing is narrowed, truncated or normalised here. All three belong to
/// `embed::pack_vector`, which is the one gate every vector in the app passes
/// through — a second copy of that logic is how two embedding spaces end up in
/// one table.
fn decode_embedding(value: &Value) -> Result<Vec<f32>, EmbedError> {
    match value {
        Value::String(encoded) => {
            let raw = base64::engine::general_purpose::STANDARD
                .decode(encoded.trim())
                .map_err(|_| EmbedError::Document { code: "embedding-not-base64".into() })?;
            if raw.len() % 4 != 0 {
                return Err(EmbedError::Document { code: "embedding-not-f32".into() });
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
                    .ok_or(EmbedError::Document { code: "embedding-not-numeric".into() })
            })
            .collect(),
        _ => Err(EmbedError::Document { code: "embedding-wrong-type".into() }),
    }
}

/// The local transport kind, never a response body.
fn transport_detail(transport: &ureq::Transport) -> String {
    match transport.message() {
        Some(message) => format!("{}: {message}", transport.kind()),
        None => transport.kind().to_string(),
    }
}

// ── The seam ─────────────────────────────────────────────────────────────────

impl RequestRun for VoyageCloud {
    /// The largest request this account can currently get accepted.
    ///
    /// **Not the API maximum**, which is the bug this method exists to prevent.
    /// A request may hold 320,000 tokens, but a 10,000 TPM account refuses
    /// anything over 10,000 with a 429 that no amount of pacing clears — so the
    /// real ceiling is whichever of the two is smaller, and it moves the moment
    /// the throttle learns the tier.
    fn max_tokens(&self) -> u64 {
        let tier = self.gate.tier().tpm.max(1.0) as u64;
        batch::MAX_TOKENS_PER_REQUEST.min(tier)
    }

    /// Embed a group of pages, in as many requests as the current ceiling
    /// allows.
    ///
    /// The batcher already packs to `max_tokens`, so on a settled account this
    /// is one request and the loop runs once. It exists for the case the
    /// batcher cannot cover: the **first** request of a fresh run is packed
    /// optimistically, and the 429 that corrects it arrives after the pages are
    /// already grouped. Repacking here is what turns that into progress.
    fn run(&self, pages: &[RenderedPage]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let costs: Vec<u64> =
            pages.iter().map(|page| batch::tokens_for(page.width, page.height)).collect();
        let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(pages.len());
        let mut offset = 0;

        while offset < pages.len() {
            // Re-read each time round: the previous chunk's 429 may have just
            // shrunk this by a factor of 32.
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
            let cost =
                Cost { tokens: spent, pixels: chunk.iter().map(batch::billed_pixels).sum() };
            match self.send(image_inputs(chunk), INPUT_TYPE_DOCUMENT, chunk.len(), cost) {
                Ok(part) => {
                    vectors.extend(part);
                    offset = end;
                }
                // The ceiling moved under us mid-request. Round again from the
                // same offset and this time the chunk is small enough.
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
        // ── Page-count agreement ─────────────────────────────────────────────
        //
        // `raster.rs` counts with pdfium; the parse record beside this PDF was
        // counted with `lopdf`, and the two can genuinely disagree — a damaged
        // xref pdfium rebuilds, a `/Count` that lies, an incremental update
        // they read different revisions of. `page_no` is the join key
        // retrieval rests on, so embedding against the wrong numbers files
        // every vector under the wrong page: silent, permanent, and only
        // visible as search results that are subtly about the wrong slide.
        //
        // Checked here, before a single pixel is billed, rather than after.
        let theirs = raster::page_count(pdf)?;
        if page_count > 0 && theirs != page_count {
            return Err(EmbedError::Document { code: "page-count-mismatch".into() });
        }
        let expected = if page_count > 0 { page_count } else { theirs };

        // Refused before the work, not during it, so a run that cannot fit
        // does not half-embed a document and pay for it.
        self.ledger.ensure_available(0)?;

        let runner: Arc<dyn RequestRun> = Arc::new(self.clone());
        let pages = batch::run_document(pdf, expected, runner, self.limits, on_progress)?;

        // **Never a partial record.** `run_document` already refuses to return
        // a short document; this is the second lock on the same door, because
        // `EmbedOutput::new` drops pages outside `1..=expected` and a backend
        // whose page numbers drifted would otherwise write a record that reads
        // as finished. `embed::is_embedded` now checks coverage, so such a
        // record would re-embed rather than lie — but it should not exist.
        let output = EmbedOutput::new(pdf, expected, pages);
        if output.page_count as u32 != expected {
            return Err(EmbedError::Document { code: "incomplete".into() });
        }
        Ok(output)
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(EmbedError::Document { code: "empty-query".into() });
        }
        // Text bills by tokens, not pixels. Four characters a token is the
        // usual rule of thumb and it is only used to pace the throttle and to
        // reserve — `usage.total_tokens` on the response is what settles it.
        let cost = Cost { tokens: (text.len() as u64 / 4) + 1, pixels: 0 };
        let inputs = json!([{ "content": [{ "type": "text", "text": text }] }]);
        let vectors = self.send(inputs, INPUT_TYPE_QUERY, 1, cost).map_err(|failure| {
            match failure {
                SendFailure::Embed(error) => error,
                // Unreachable: `send` only asks to be repacked when there is
                // more than one input, and a query is one piece of text.
                SendFailure::Resize => EmbedError::RateLimited { retry_after_secs: None },
            }
        })?;
        let raw = vectors.into_iter().next().ok_or(EmbedError::Document {
            code: "embedding-missing".into(),
        })?;
        // Through the seam's gate, so a query and a stored page are normalised
        // by the same code and the dot product between them is a cosine.
        Ok(unpack_vector(&pack_vector(&raw)?))
    }

    /// Ready means: a key (which `new` guarantees) **and a renderer**.
    ///
    /// The renderer check is the load-bearing half. A missing libpdfium
    /// condemns every file in the run, and the seam's error vocabulary has no
    /// variant that is both latching and fixable — so it is caught here, where
    /// `embed::preflight` refuses the whole run once, instead of failing two
    /// hundred files with the same message.
    ///
    /// Quota is deliberately *not* readiness: a spent allowance is a
    /// `QuotaExhausted` on the call that hits it, which says something true
    /// about waiting, where `NotReady` would send the UI to "try again in a
    /// moment".
    fn health(&self) -> Health {
        Health {
            backend: BACKEND.to_string(),
            model: EMBED_MODEL.to_string(),
            dim: EMBED_DIM,
            ready: raster::available().is_ok(),
        }
    }
}

/// What the account has spent and what tier it turned out to be on, for the
/// settings page. Mirrors `parse::mineru::client::usage_status`.
pub fn usage_status() -> Value {
    let usage = UsageLedger::shared().snapshot();
    json!({
        "requests": usage.requests,
        "tokens": usage.tokens,
        "pixels": usage.pixels,
        "quota_exhausted": usage.latched(),
        "rpm": usage.tier.rpm,
        "tpm": usage.tier.tpm,
        // How much to trust the two numbers above: "stated" is Voyage's own
        // answer, "observed" is inferred from behaviour, "assumed" is the
        // opening guess nothing has corrected yet.
        "tier_source": usage.tier.source.as_str(),
        "tier_learned_at": usage.tier.learned_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::super::ledger::{hold, Tier, TierSource, FREE_TPM};

    // ── A Voyage that is not Voyage ──────────────────────────────────────────
    //
    // Every test below drives the real protocol against a `tiny_http` server on
    // loopback. Nothing here ever reaches api.voyageai.com; the base URL is
    // configuration, which is exactly why it is configuration.

    #[derive(Clone)]
    struct Hit {
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl Hit {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers.iter().find(|(f, _)| f == name).map(|(_, v)| v.as_str())
        }

        fn json(&self) -> Value {
            serde_json::from_slice(&self.body).unwrap_or(Value::Null)
        }
    }

    struct Reply {
        status: u16,
        body: Vec<u8>,
        headers: Vec<(String, String)>,
    }

    impl Reply {
        fn json(value: Value) -> Self {
            Self { status: 200, body: value.to_string().into_bytes(), headers: Vec::new() }
        }

        fn status(status: u16, value: Value) -> Self {
            Self { status, body: value.to_string().into_bytes(), headers: Vec::new() }
        }

        fn with_header(mut self, name: &str, value: &str) -> Self {
            self.headers.push((name.into(), value.into()));
            self
        }
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
            H: Fn(&Hit, usize) -> Reply + Send + 'static,
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
                        let mut body = Vec::new();
                        std::io::Read::read_to_end(request.as_reader(), &mut body).ok();
                        let hit = Hit {
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
                        };
                        let index = {
                            let mut log = hold(&hits);
                            log.push(hit.clone());
                            log.len() - 1
                        };
                        let reply = handler(&hit, index);
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
            Self { port, hits, stop, handle: Some(handle) }
        }

        fn base(&self) -> String {
            format!("http://127.0.0.1:{}/v1", self.port)
        }

        fn hits(&self) -> Vec<Hit> {
            hold(&self.hits).clone()
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

    struct Scratch {
        root: PathBuf,
    }

    impl Scratch {
        fn new(name: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            let root = std::env::temp_dir().join(format!("oculus-voyage-{name}-{stamp}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn join(&self, name: &str) -> PathBuf {
            self.root.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).ok();
        }
    }

    /// The test key. Deliberately a value that could never be a real one, and
    /// the only "key" string anywhere in this file: the live key lives in the
    /// keychain and is never written to a file, a test or a log.
    const TEST_KEY: &str = "pa-test-only-not-a-real-key";

    /// The gates in these tests run 500x fast. Every decision is the real one;
    /// only the clock is compressed, which is what makes a faithful 3 RPM /
    /// 10K TPM run testable at all.
    const TEST_PACE: f64 = 500.0;

    /// The body Voyage actually sends on this account, verbatim from a live
    /// 429 — and it arrives with **no `Retry-After` header**, which is why the
    /// fallback wait is the path under test rather than a safety net.
    const LIVE_FREE_TIER_429: &str = "You have not yet added your payment method in the \
        billing page and will have reduced rate limits of 3 RPM and 10K TPM. To unlock our \
        standard rate limits, please add a payment method in the billing page...";

    fn client(fake: &Fake, scratch: &Scratch) -> VoyageCloud {
        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        VoyageCloud::new(&fake.base(), TEST_KEY)
            .unwrap()
            .with_ledger(ledger.clone())
            // A private gate at tier-1 numbers so one test cannot pace another,
            // and a scale that turns every wait into milliseconds.
            .with_gate(Arc::new(RateGate::with_pace(ledger, Duration::from_secs(3_600), TEST_PACE)))
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

    fn ok_response(count: usize) -> Reply {
        let data: Vec<Value> = (0..count)
            .map(|index| json!({ "index": index, "embedding": wire_vector(index as u32) }))
            .collect();
        Reply::json(json!({
            "object": "list",
            "data": data,
            "model": EMBED_MODEL,
            "usage": { "total_tokens": 3_572 * count },
        }))
    }

    /// A real PDF, because `raster.rs` renders it with pdfium and a stub file
    /// would not do.
    fn write_pdf(path: &Path, pages: usize) {
        // Small on purpose: these render fast and their token cost is
        // irrelevant to what most of the tests are checking.
        write_pdf_sized(path, pages, 144, 144);
    }

    fn write_pdf_sized(path: &Path, pages: usize, width: i64, height: i64) {
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
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => kids, "Count" => count,
            }),
        );
        let catalog = document.add_object(dictionary! {
            "Type" => "Catalog", "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog);
        document.save(path).unwrap();
    }

    /// A landscape-A4 deck — 842 x 595 pt, which renders to 2339 x 1653 at
    /// `RENDER_DPI` and is the page every token figure in this module was
    /// measured against.
    fn write_a4_pdf(path: &Path, pages: usize) {
        write_pdf_sized(path, pages, 842, 595);
    }

    /// The end-to-end tests need libpdfium; they skip rather than fail on a
    /// checkout that has not run `bun run pdfium`, exactly as `raster.rs` does.
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
        let fake = Fake::start(|_, _| ok_response(2));
        let scratch = Scratch::new("body");
        let client = client(&fake, &scratch);

        let pages = vec![
            RenderedPage { page_no: 1, width: 100, height: 100, png: b"\x89PNGone".to_vec() },
            RenderedPage { page_no: 2, width: 100, height: 100, png: b"\x89PNGtwo".to_vec() },
        ];
        let vectors = client.run(&pages).unwrap();
        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0].len(), EMBED_DIM);

        let hit = &fake.hits()[0];
        let body = hit.json();
        assert_eq!(body["model"], EMBED_MODEL);
        assert_eq!(body["input_type"], "document");
        assert_eq!(body["output_dimension"], EMBED_DIM);
        // Transport, not precision — and `output_dtype` must not appear, which
        // is the parameter this is forever confused with.
        assert_eq!(body["output_encoding"], "base64");
        assert!(body.get("output_dtype").is_none(), "{body}");

        let first = &body["inputs"][0]["content"][0];
        assert_eq!(first["type"], "image_base64");
        assert!(
            first["image_base64"].as_str().unwrap().starts_with("data:image/png;base64,"),
            "{first}"
        );
        assert_eq!(hit.header("authorization"), Some(format!("Bearer {TEST_KEY}").as_str()));
    }

    #[test]
    fn a_query_uses_the_other_side_of_the_asymmetry() {
        let fake = Fake::start(|_, _| ok_response(1));
        let scratch = Scratch::new("query");
        let client = client(&fake, &scratch);

        let vector = client.embed_query("what is a martingale").unwrap();
        assert_eq!(vector.len(), EMBED_DIM);
        // Normalised through the seam's gate, so a dot product against a stored
        // page is a cosine.
        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 2e-3, "{norm}");

        let body = fake.hits()[0].json();
        // The whole point: documents and queries must use opposite sides.
        assert_eq!(body["input_type"], "query");
        assert_eq!(body["inputs"][0]["content"][0]["type"], "text");
    }

    // ── Decoding ─────────────────────────────────────────────────────────────

    #[test]
    fn base64_is_f32_little_endian_at_two_thousand_and_forty_eight_bytes() {
        // The measurement this whole encoding choice rests on.
        let encoded = wire_vector(0);
        let raw = base64::engine::general_purpose::STANDARD.decode(&encoded).unwrap();
        assert_eq!(raw.len(), 2_048);
        assert_eq!(raw.len(), EMBED_DIM * 4);

        let decoded = decode_embedding(&Value::String(encoded)).unwrap();
        assert_eq!(decoded.len(), EMBED_DIM);
        assert_eq!(decoded[0], -48.0 / 64.0);
        assert_eq!(decoded[1], (31.0 - 48.0) / 64.0);

        // Nothing here narrows or normalises — that is `pack_vector`'s job,
        // and the raw floats must reach it untouched. ‖v‖ here is ~9.8, not 1.
        let norm = decoded.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!(norm > 2.0, "decode must not normalise: {norm}");
    }

    #[test]
    fn a_json_float_array_still_decodes() {
        // If a proxy or a changed default drops `output_encoding`, the client
        // degrades to slow rather than broken.
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
        // A reordered `data` array would otherwise put page 7's vector under
        // page 3 — invisible, permanent, and fatal to the join key.
        let payload = json!({
            "data": [
                { "index": 1, "embedding": wire_vector(11) },
                { "index": 0, "embedding": wire_vector(0) },
            ],
        });
        let vectors = decode_response(&payload, 2).unwrap();
        assert_eq!(vectors[0], decode_embedding(&json!(wire_vector(0))).unwrap());
        assert_eq!(vectors[1], decode_embedding(&json!(wire_vector(11))).unwrap());

        // A short answer, a repeated index and an out-of-range index are all
        // refused rather than silently filled.
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
        let fake = Fake::start(|_, _| {
            Reply::status(401, json!({ "detail": "Provided API key is invalid." }))
        });
        let scratch = Scratch::new("auth");
        let client = client(&fake, &scratch);

        let error = client.embed_query("anything").unwrap_err();
        assert_eq!(error.kind(), "rejected_credentials");
        assert!(!error.retryable());
        assert!(error.latching(), "every other file would hit the same rejection");
        assert_eq!(fake.hits().len(), 1, "a rejected key cannot be retried into working");
    }

    #[test]
    fn an_expired_key_says_so() {
        let fake = Fake::start(|_, _| {
            Reply::status(403, json!({ "detail": "This API key has expired." }))
        });
        let scratch = Scratch::new("expired");
        let error = client(&fake, &scratch).embed_query("x").unwrap_err();
        assert!(matches!(error, EmbedError::RejectedCredentials { expired: true, .. }), "{error:?}");
    }

    #[test]
    fn money_latches_the_ledger_but_pace_does_not() {
        let fake = Fake::start(|_, _| {
            Reply::status(403, json!({ "detail": "Your account has run out of credit." }))
        });
        let scratch = Scratch::new("credit");
        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        let client = VoyageCloud::new(&fake.base(), TEST_KEY)
            .unwrap()
            .with_ledger(ledger.clone())
            .with_gate(Arc::new(RateGate::with_pace(ledger.clone(), Duration::from_secs(3_600), TEST_PACE)))
            .with_time_scale(0.002);

        let error = client.embed_query("x").unwrap_err();
        assert_eq!(error.kind(), "quota_exhausted");
        assert!(error.latching(), "every file draws on the same allowance");
        assert!(error.retryable(), "and it repairs itself when the account is topped up");
        // The server's own answer outranks the local guess, so nothing else
        // goes near the network.
        assert!(ledger.snapshot().latched());
        assert!(matches!(ledger.ensure_available(0), Err(EmbedError::QuotaExhausted)));
    }

    #[test]
    fn a_429_is_waited_out_and_its_stated_limit_is_learned() {
        let fake = Fake::start(|_, index| {
            if index == 0 {
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
        let scratch = Scratch::new("throttle");
        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        let gate =
            Arc::new(RateGate::with_pace(ledger.clone(), Duration::from_secs(3_600), TEST_PACE));
        let client = VoyageCloud::new(&fake.base(), TEST_KEY)
            .unwrap()
            .with_ledger(ledger.clone())
            .with_gate(gate.clone())
            .with_time_scale(0.002);

        // Succeeds. The 429 is a wait, not a failure — which on the free
        // programme is the difference between "slow" and "unusable".
        client.embed_query("hello").unwrap();
        assert_eq!(fake.hits().len(), 2);

        // And the account's real programme has been discovered, not asked for.
        let tier = gate.tier();
        assert_eq!(tier.source, TierSource::Stated);
        assert_eq!(tier.tpm, FREE_TPM);
        // Written down, so the next run starts where this one finished.
        assert_eq!(ledger.tier().tpm, FREE_TPM);
    }

    #[test]
    fn an_endless_429_eventually_becomes_a_retryable_error_rather_than_a_parked_thread() {
        let fake = Fake::start(|_, _| {
            Reply::status(429, json!({ "detail": "slow down" })).with_header("Retry-After", "1")
        });
        let scratch = Scratch::new("endless");
        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        // A tiny time scale makes the 30-minute deadline milliseconds.
        let client = VoyageCloud::new(&fake.base(), TEST_KEY)
            .unwrap()
            .with_ledger(ledger.clone())
            .with_gate(Arc::new(RateGate::with_pace(ledger, Duration::from_secs(3_600), TEST_PACE)))
            .with_time_scale(0.000_02);

        let error = client.embed_query("x").unwrap_err();
        assert_eq!(error.kind(), "rate_limited");
        // Retryable and, emphatically, not latching: on the free programme this
        // is the steady state of a working run.
        assert!(error.retryable());
        assert!(!error.latching());
    }

    #[test]
    fn a_server_fault_is_retried_and_then_reported_as_transport() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let seen = attempts.clone();
        let fake = Fake::start(move |_, _| {
            seen.fetch_add(1, Ordering::SeqCst);
            Reply::status(503, json!({ "detail": "upstream" }))
        });
        let scratch = Scratch::new("fault");
        let error = client(&fake, &scratch).embed_query("x").unwrap_err();

        assert_eq!(error.kind(), "offline");
        // Not `document`: a server fault must not mark this file permanently
        // failed.
        assert!(error.retryable());
        assert_eq!(attempts.load(Ordering::SeqCst), ATTEMPTS as usize);
    }

    // ── Nothing leaks ────────────────────────────────────────────────────────

    #[test]
    fn an_error_never_carries_the_key_a_url_or_the_server_body() {
        // The realistic hazard: this API's error bodies can echo the request,
        // which for a page request means an echo of the image, and on a 429 the
        // account's billing state.
        let fake = Fake::start(|_, _| {
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
        let scratch = Scratch::new("leak");
        let error = client(&fake, &scratch).embed_query("x").unwrap_err();

        for rendering in [error.to_string(), format!("{error:?}")] {
            assert!(!rendering.contains(TEST_KEY), "{rendering}");
            assert!(!rendering.contains("SECRET"), "{rendering}");
            assert!(!rendering.contains("signed.example"), "{rendering}");
            // No URL of any kind — the `http-400` code below is a status, not
            // an address.
            assert!(!rendering.contains("://"), "{rendering}");
            assert!(!rendering.contains("base64"), "{rendering}");
        }
        // What it does say is the status, which is all anyone needs.
        assert!(format!("{error:?}").contains("400"), "{error:?}");
    }

    // ── A whole document ─────────────────────────────────────────────────────

    #[test]
    fn a_document_embeds_every_page_and_reports_real_progress() {
        if !renderer_present() {
            return;
        }
        let fake = Fake::start(|hit, _| {
            let inputs = hit.json()["inputs"].as_array().map(Vec::len).unwrap_or(0);
            ok_response(inputs)
        });
        let scratch = Scratch::new("document");
        let pdf = scratch.join("deck.pdf");
        write_pdf(&pdf, 5);

        // Two pages per request, so the batching path is actually exercised.
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
        assert_eq!(output.pages.iter().map(|p| p.page_no).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
        assert_eq!(output.model, EMBED_MODEL);
        assert_eq!(output.dim, EMBED_DIM);
        assert_eq!(fake.hits().len(), 3, "5 pages at 2 per request");

        // Summed from finished requests, never inferred, and monotonic.
        let seen = hold(&progress).clone();
        assert_eq!(seen.last().copied(), Some(5));
        assert!(seen.windows(2).all(|pair| pair[0] <= pair[1]), "{seen:?}");
    }

    #[test]
    fn a_document_that_could_not_embed_every_page_writes_nothing() {
        if !renderer_present() {
            return;
        }
        // The second request fails. The first one's vectors are real, paid for,
        // and must still not become a record: `is_embedded` would then have to
        // notice the shortfall, and a page that is silently never searchable is
        // the worst outcome this pipeline has.
        let fake = Fake::start(|hit, index| {
            let inputs = hit.json()["inputs"].as_array().map(Vec::len).unwrap_or(0);
            if index == 1 {
                Reply::status(400, json!({ "detail": "no" }))
            } else {
                ok_response(inputs)
            }
        });
        let scratch = Scratch::new("partial");
        let pdf = scratch.join("deck.pdf");
        write_pdf(&pdf, 4);

        let client = client(&fake, &scratch).with_limits(Limits {
            max_inputs: 1,
            max_tokens: batch::MAX_TOKENS_PER_REQUEST,
            // Serial, so "the second request" means the second one.
            in_flight: 1,
        });
        let error = client.embed(&pdf, 4, &|_| {}).unwrap_err();
        assert_eq!(error.kind(), "document");
        assert!(!error.latching(), "one bad document must not condemn the run");
        // Nothing on disk: the seam's writer is never reached.
        assert!(!crate::embed::emb_path(&pdf).exists());
    }

    #[test]
    fn a_free_tier_account_shrinks_its_requests_instead_of_retrying_forever() {
        if !renderer_present() {
            return;
        }
        // A server that behaves the way the live one does: anything over the
        // account's 10,000 TPM is refused outright, with the verbatim body and
        // no `Retry-After`. A client that only *slowed down* would send the
        // same doomed request once every twenty seconds for ever.
        let fake = Fake::start(|hit, _| {
            let inputs = hit.json()["inputs"].as_array().map(Vec::len).unwrap_or(0);
            // 842 x 595 pt at RENDER_DPI, capped at 2M billed pixels.
            let tokens = inputs as u64 * 3_572;
            if tokens > 10_000 {
                Reply::status(429, json!({ "detail": LIVE_FREE_TIER_429 }))
            } else {
                ok_response(inputs)
            }
        });
        let scratch = Scratch::new("livelock");
        let pdf = scratch.join("deck.pdf");
        write_a4_pdf(&pdf, 4);

        let ledger = Arc::new(UsageLedger::at(scratch.join("voyage-usage.json")));
        let gate =
            Arc::new(RateGate::with_pace(ledger.clone(), Duration::from_secs(3_600), TEST_PACE));
        let client = VoyageCloud::new(&fake.base(), TEST_KEY)
            .unwrap()
            .with_ledger(ledger.clone())
            .with_gate(gate.clone())
            .with_time_scale(0.002);

        // Packed optimistically, this is one request of 4 x 3,572 = 14,288
        // tokens — within a whisker of the ~14,284 that was measured being
        // refused live.
        assert_eq!(client.max_tokens(), batch::MAX_TOKENS_PER_REQUEST);

        let output = client.embed(&pdf, 4, &|_| {}).expect("the run must make progress");
        assert_eq!(output.page_count, 4);
        assert_eq!(output.pages.iter().map(|p| p.page_no).collect::<Vec<_>>(), vec![1, 2, 3, 4]);

        // The ceiling was learned, not assumed, and the requests got *smaller*.
        assert_eq!(gate.tier().tpm, 10_000.0);
        assert_eq!(gate.tier().rpm, 3.0);
        assert_eq!(client.max_tokens(), 10_000);

        // One refusal, then two accepted halves — never the same size twice.
        let sizes: Vec<usize> = fake
            .hits()
            .iter()
            .map(|hit| hit.json()["inputs"].as_array().map(Vec::len).unwrap_or(0))
            .collect();
        assert_eq!(sizes, vec![4, 2, 2], "{sizes:?}");
    }

    #[test]
    fn a_single_page_is_never_too_big_to_shrink_to() {
        // The floor the repacking rests on: the 2M billing cap puts *any* page
        // at 3,572 tokens, and the slowest programme Voyage runs is 10,000 TPM.
        // So shrinking always terminates on something acceptable — which was
        // not true before the cap, when a page could estimate at 28,571.
        assert!(batch::tokens_for(u32::MAX, u32::MAX) < 10_000);
    }

    #[test]
    fn a_page_count_the_two_counters_disagree_on_is_a_document_error() {
        if !renderer_present() {
            return;
        }
        let fake = Fake::start(|_, _| ok_response(1));
        let scratch = Scratch::new("count");
        let pdf = scratch.join("deck.pdf");
        write_pdf(&pdf, 3);

        // The parse record says four pages; pdfium finds three. `page_no` is
        // the join key retrieval rests on, so this is refused rather than
        // embedded against the wrong numbers.
        let error = client(&fake, &scratch).embed(&pdf, 4, &|_| {}).unwrap_err();
        assert_eq!(error.kind(), "document");
        assert!(format!("{error:?}").contains("page-count-mismatch"), "{error:?}");
        // Refused *before* a single pixel was billed.
        assert!(fake.hits().is_empty());
    }

    #[test]
    fn a_latching_failure_stops_the_document_rather_than_embedding_the_rest_of_it() {
        if !renderer_present() {
            return;
        }
        let fake = Fake::start(|_, _| {
            Reply::status(401, json!({ "detail": "Provided API key is invalid." }))
        });
        let scratch = Scratch::new("latch");
        let pdf = scratch.join("deck.pdf");
        write_pdf(&pdf, 6);

        let client = client(&fake, &scratch).with_limits(Limits {
            max_inputs: 1,
            max_tokens: batch::MAX_TOKENS_PER_REQUEST,
            in_flight: 1,
        });
        let error = client.embed(&pdf, 6, &|_| {}).unwrap_err();
        assert!(error.latching(), "{error:?}");
        // The run stopped early rather than paying for five more refusals.
        assert!(fake.hits().len() < 6, "{} requests", fake.hits().len());
    }

    #[test]
    fn health_refuses_the_run_when_the_renderer_is_missing() {
        let fake = Fake::start(|_, _| ok_response(1));
        let scratch = Scratch::new("health");
        let health = client(&fake, &scratch).health();
        assert_eq!(health.model, EMBED_MODEL);
        assert_eq!(health.dim, EMBED_DIM);
        assert_eq!(health.ready, raster::available().is_ok());
        if health.ready {
            health.check().unwrap();
        } else {
            // One refusal for the whole run, not one per file.
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

    /// The one test that talks to Voyage, and it is off unless you ask.
    ///
    /// Set `OCULUS_VOYAGE_LIVE=1` to run it. It embeds **three small pages in
    /// one request** using the key already in the keychain — the key is never
    /// read by this file, written to it, or printed: `from_config` fetches it
    /// through `CredentialSource::Keychain` and it never leaves that path.
    ///
    /// Keep it small and keep it once. With no payment method the account is
    /// 3 RPM / 10K TPM, so a document-sized run here would spend most of an
    /// hour inside 429s. A 429 *is* the expected answer on this machine and it
    /// is not a bug to chase — the run retries through it, which is the
    /// behaviour being verified.
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

        let scratch = Scratch::new("live");
        let pdf = scratch.join("live.pdf");
        // 144 x 144 pt is 400 x 400 px at RENDER_DPI — ~286 tokens a page, so
        // three of them are under a tenth of the free minute's budget.
        write_pdf(&pdf, 3);

        let output = client.embed(&pdf, 3, &|update| {
            eprintln!("live: {}/{}", update.pages_done, update.total_pages);
        });
        let output = output.expect("live embed");

        assert_eq!(output.page_count, 3);
        assert_eq!(output.dim, EMBED_DIM);
        assert_eq!(output.model, EMBED_MODEL);
        for page in &output.pages {
            let vector = page.vector_f32().unwrap();
            assert_eq!(vector.len(), EMBED_DIM);
            let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 2e-3, "page {}: ‖v‖ = {norm}", page.page_no);
        }
        // What the account turned out to be, discovered rather than declared.
        eprintln!("live: {}", usage_status());
    }

    #[test]
    fn the_usage_status_never_carries_a_key() {
        let status = usage_status();
        assert!(status.get("tier_source").is_some(), "{status}");
        assert!(!status.to_string().contains("pa-"), "{status}");
        // The tier is reported with how much to trust it, which is the whole
        // point of detecting it rather than asking.
        for source in [TierSource::Assumed, TierSource::Observed, TierSource::Stated] {
            assert!(!source.as_str().is_empty());
        }
        assert_eq!(Tier::free().source, TierSource::Stated);
    }
}

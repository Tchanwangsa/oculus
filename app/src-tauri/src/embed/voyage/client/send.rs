//! One request: reserve, wait for the gate, send, and learn from a 429.

use super::wait::{WaitNotice, REPORTED_WAIT};
use super::wire::decode_response;
use super::VoyageCloud;
use crate::embed::voyage::batch::RequestRun;
use crate::embed::voyage::ledger::is_about_credit;
use crate::embed::{EmbedError, Limiter, Wait, EMBED_DIM, EMBED_MODEL};
use crate::providers::ratelimit::{nap, transport_detail, Retry};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// `/embeddings` is the text-only model's and refuses images.
const EMBED_PATH: &str = "/multimodalembeddings";

/// Attempts per request. A 429 deliberately does not consume one.
pub(super) const ATTEMPTS: u32 = 4;
/// Generous: a full request is tens of MB of base64 page images.
const API_TIMEOUT: Duration = Duration::from_secs(300);

/// How long one request may spend throttled before it becomes a retryable
/// `RateLimited`. Minutes of 429s are routine, but a zero limit or a proxy
/// answering 429 to everything must not park a thread for ever.
const THROTTLE_DEADLINE: Duration = Duration::from_secs(30 * 60);

/// What one request is about to cost, reserved before it is sent.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Cost {
    pub(super) tokens: u64,
    pub(super) pixels: u64,
}

/// Why `send` came back without vectors. `Resize` is not a failure: a 429
/// showed the account's ceiling is below this request, which no pace can fix,
/// so it is handed back to be repacked smaller.
pub(super) enum SendFailure {
    Embed(EmbedError),
    Resize,
}

impl From<EmbedError> for SendFailure {
    fn from(error: EmbedError) -> Self {
        SendFailure::Embed(error)
    }
}

impl VoyageCloud {
    /// Send one request and hand back one vector per input: reserve, wait for
    /// the gate, send, and on a 429 learn the tier and go round again.
    /// `on_wait` hears of each rate-limit wait long enough to look like a
    /// stall, and `None` once the request is admitted.
    pub(super) fn send(
        &self,
        inputs: Value,
        input_type: &str,
        expected: usize,
        cost: Cost,
        on_wait: &dyn Fn(Option<Wait>),
    ) -> Result<Vec<Vec<f32>>, SendFailure> {
        let url = format!("{}{}", self.base_url, EMBED_PATH);
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
                Err(ureq::Error::Transport(transport)) => {
                    if retry.back_off() {
                        continue;
                    }
                    return Err(EmbedError::Offline(transport_detail(&transport)).into());
                }
            };

            let text = response.into_string().unwrap_or_default();
            let payload = match serde_json::from_str::<Value>(&text) {
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

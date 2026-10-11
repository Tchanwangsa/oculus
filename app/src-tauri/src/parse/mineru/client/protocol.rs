//! The API calls: JSON requests, file uploads and result downloads.

use super::errors::{auth_error, check_transfer_url, safe_code};
use super::route::Unanswered;
use super::upload::UploadBody;
use super::MinerUCloud;
use super::{ATTEMPTS, TRANSFER_STALL, UPLOAD_CHUNK, UPLOAD_REPORT_EVERY};
use crate::parse::mineru::result_tls;
use crate::parse::{parse_config, ParseError};
use crate::providers::ratelimit::{nap, transport_detail, Retry, TokenBucket};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::time::Duration;

impl MinerUCloud {
    /// One API call, with the shared retry policy. A 429 sleeps `Retry-After`
    /// (clamped to 1..60s) **without consuming an attempt** — per-minute
    /// pressure is a wait, not a failure — so only `POLL_DEADLINE` bounds it.
    pub(super) fn api_json(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        bucket: &TokenBucket,
    ) -> Result<Value, ParseError> {
        // `ureq`'s `json` feature is off, so the body is encoded here.
        let encoded = body.map(|value| serde_json::to_vec(&value).unwrap_or_default());
        let mut retry = Retry::new(ATTEMPTS, self.time_scale);

        while retry.attempts_left() {
            bucket.acquire();
            let response = match self.request(method, path, encoded.as_deref()) {
                Ok(response) => response,
                Err(Unanswered::Fatal(error)) => return Err(error),
                Err(Unanswered::Transport(detail)) => {
                    if retry.back_off() {
                        continue;
                    }
                    return Err(ParseError::Offline(detail));
                }
            };

            match response.status {
                429 => {
                    let wait = response
                        .header("Retry-After")
                        .and_then(|value| value.trim().parse::<f64>().ok())
                        .unwrap_or(60.0)
                        .clamp(1.0, 60.0);
                    nap(Duration::from_secs_f64(wait), self.time_scale);
                    continue;
                }
                401 | 403 => return Err(auth_error(&String::from_utf8_lossy(&response.body))),
                status if status >= 400 => {
                    if status >= 500 && retry.back_off() {
                        continue;
                    }
                    return Err(ParseError::Document {
                        code: format!("http-{status}"),
                    });
                }
                _ => {}
            }

            let payload = match serde_json::from_slice::<Value>(&response.body).ok() {
                Some(payload) => payload,
                None if retry.back_off() => continue,
                None => return Err(ParseError::Offline("unreadable response".into())),
            };
            if !payload.is_object() {
                return Err(ParseError::Document {
                    code: "invalid-response".into(),
                });
            }

            let code = payload.get("code");
            if code.and_then(Value::as_i64) == Some(0) {
                return Ok(payload.get("data").cloned().unwrap_or_else(|| json!({})));
            }
            if matches!(code.and_then(Value::as_str), Some("A0202" | "A0211")) {
                return Err(auth_error(&payload.to_string()));
            }
            if code.and_then(Value::as_i64) == Some(-60018) {
                self.ledger.latch_exhausted();
                return Err(ParseError::QuotaExhausted);
            }
            if matches!(code.and_then(Value::as_i64), Some(-60009 | -10001 | -60007))
                && retry.back_off()
            {
                continue;
            }
            // Only the code. The message beside it can quote a signed URL.
            return Err(ParseError::Document {
                code: safe_code(code),
            });
        }
        Err(ParseError::Offline("request failed".into()))
    }

    /// `PUT` the file to a signed URL. **No `Content-Type`**: MinerU rejects
    /// the upload when one is present (`ureq` adds none unless asked). An
    /// explicit `Content-Length` avoids chunked encoding, which a signed PUT
    /// rejects. No `Authorization` and no retry: the signature is single-use.
    /// `report` gets the bytes sent so far; once `cancelled` holds, the body
    /// stops reading and the PUT ends as `Cancelled`.
    pub(super) fn put_file(
        &self,
        url: &str,
        path: &Path,
        report: &dyn Fn(u64),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), ParseError> {
        check_transfer_url(url, "upload")?;
        let size = fs::metadata(path)
            .map_err(|e| ParseError::Io(format!("stat {}: {e}", path.display())))?
            .len();
        let file = fs::File::open(path)
            .map_err(|e| ParseError::Io(format!("open {}: {e}", path.display())))?;

        let agent = ureq::AgentBuilder::new()
            .timeout_connect(TRANSFER_STALL)
            .timeout_read(TRANSFER_STALL)
            .timeout_write(TRANSFER_STALL)
            // A redirected signed PUT has lost its signature.
            .redirects(0)
            .build();
        let body = UploadBody::new(
            BufReader::with_capacity(UPLOAD_CHUNK, file),
            size,
            UPLOAD_REPORT_EVERY,
            report,
            cancelled,
        );
        match agent
            .put(url)
            .set("Content-Length", &size.to_string())
            .send(body)
        {
            Err(_) if cancelled() => Err(ParseError::Cancelled),
            Ok(response) if (200..300).contains(&response.status()) => Ok(()),
            Ok(response) => Err(ParseError::Document {
                code: format!("upload-http-{}", response.status()),
            }),
            Err(ureq::Error::Status(status, _)) => Err(ParseError::Document {
                code: format!("upload-http-{status}"),
            }),
            Err(ureq::Error::Transport(transport)) => {
                Err(ParseError::Offline(transport_detail(&transport)))
            }
        }
    }

    pub(super) fn download_zip(&self, url: &str, destination: &Path) -> Result<(), ParseError> {
        check_transfer_url(url, "result")?;
        let mut last = ParseError::Offline("result download failed".into());
        for attempt in 0..3u32 {
            let agent = result_tls::agent(parse_config().accept_expired_result_cert);
            let attempted = agent
                .get(url)
                .call()
                .and_then(|response| Ok((response.status(), response.into_reader())));
            match attempted {
                Ok((status, mut reader)) if (200..300).contains(&status) => {
                    let file = fs::File::create(destination).map_err(|e| {
                        ParseError::Io(format!("create {}: {e}", destination.display()))
                    })?;
                    let mut out = BufWriter::with_capacity(UPLOAD_CHUNK, file);
                    return std::io::copy(&mut reader, &mut out)
                        .map(|_| ())
                        .map_err(|e| ParseError::Io(format!("download: {e}")));
                }
                Ok((status, _)) => {
                    last = ParseError::Document {
                        code: format!("result-http-{status}"),
                    }
                }
                Err(ureq::Error::Status(status, _)) => {
                    last = ParseError::Document {
                        code: format!("result-http-{status}"),
                    }
                }
                Err(ureq::Error::Transport(transport)) => {
                    last = ParseError::Offline(transport_detail(&transport))
                }
            }
            if attempt < 2 {
                nap(Duration::from_secs(1 << attempt), self.time_scale);
            }
        }
        Err(last)
    }
}

//! Whisper on Groq: one multipart upload per audio file, timed segments back.
//! The key is `crate::groq`'s; this is the client that spends it. With
//! `oculus-keyd` installed the upload goes through keyd, which adds the key;
//! otherwise straight to Groq with the keychain's key (`super::groq_engine`).
//! Both routes hand `transcribe` the same `RawResponse`.

use std::path::Path;

use crate::credentials::{Credentialed, KeydError, RawResponse};

use super::{audio, Engine, EngineError, Segment};

const BASE_URL: &str = "https://api.groq.com/openai/v1";
/// `BASE_URL`'s path, which keyd's `forward` is given in place of a URL.
const CLOUD_PATH: &str = "/openai/v1";
const TRANSCRIPTIONS_PATH: &str = "/audio/transcriptions";
const MODEL: &str = "whisper-large-v3-turbo";

/// Groq's free tier refuses uploads over 25 MB. Spans are cut by time, so a
/// dense stretch of speech can run past its share; the margin absorbs that.
const UPLOAD_BUDGET: u64 = 20_000_000;

/// How the upload gets its key.
pub(super) enum Auth {
    /// This process holds the key: keyd is absent.
    Direct(String),
    /// keyd adds it; the key never enters this process.
    Keyd(Credentialed),
}

pub(super) struct Groq {
    auth: Auth,
    base_url: String,
    /// An ISO-639-1 code; `None` lets Groq detect the language.
    language: Option<String>,
}

impl Groq {
    pub(super) fn new(auth: Auth, language: Option<String>) -> Self {
        Self {
            auth,
            base_url: BASE_URL.into(),
            language,
        }
    }

    /// One POST of `body` by this client's route. A status is an answer,
    /// whatever it is; `transcribe` reads it. No timeout on either route: an
    /// hour of audio is a long upload and a long answer.
    fn post(&self, content_type: &str, body: &[u8]) -> Result<RawResponse, EngineError> {
        match &self.auth {
            Auth::Keyd(broker) => broker
                .send(
                    crate::groq::SECRET,
                    "POST",
                    &format!("{CLOUD_PATH}{TRANSCRIPTIONS_PATH}"),
                    &[("Content-Type", content_type)],
                    body,
                    None,
                )
                .map_err(keyd_error),
            Auth::Direct(key) => {
                let sent = ureq::post(&format!("{}{TRANSCRIPTIONS_PATH}", self.base_url))
                    .set("Authorization", &format!("Bearer {key}"))
                    .set("Content-Type", content_type)
                    .send_bytes(body);
                let response = match sent {
                    Ok(response) | Err(ureq::Error::Status(_, response)) => response,
                    Err(error) => {
                        return Err(EngineError::Failed(format!(
                            "could not reach Groq: {error}"
                        )))
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
                // A refusal's unreadable body is still a refusal.
                let body = match response.into_string() {
                    Ok(text) => text.into_bytes(),
                    Err(e) if status < 400 => {
                        return Err(EngineError::Failed(format!(
                            "Groq's reply could not be read: {e}"
                        )))
                    }
                    Err(_) => Vec::new(),
                };
                Ok(RawResponse {
                    status,
                    headers,
                    body,
                })
            }
        }
    }
}

/// keyd's refusals in Groq's words. Only `upstream` means keyd could not
/// reach Groq; every other kind is keyd's own failure.
fn keyd_error(error: KeydError) -> EngineError {
    EngineError::Failed(match error {
        KeydError::Keychain(e) => {
            format!("The keychain refused to give out the Groq API key ({e})")
        }
        KeydError::Missing(_) => super::NO_GROQ_KEY.to_string(),
        KeydError::Upstream(detail) => format!("could not reach Groq: {detail}"),
        KeydError::Absent => "oculus-keyd, which holds the Groq API key, stopped listening".into(),
        other => {
            format!(
                "oculus-keyd, which holds the Groq API key, could not send this request: {other}"
            )
        }
    })
}

impl Engine for Groq {
    fn name(&self) -> &'static str {
        "groq"
    }

    fn max_upload_bytes(&self) -> Option<u64> {
        Some(UPLOAD_BUDGET)
    }

    fn transcribe(&self, audio: &Path) -> Result<Vec<Segment>, EngineError> {
        let bytes = std::fs::read(audio)
            .map_err(|e| EngineError::Failed(format!("could not read the extracted audio: {e}")))?;
        let boundary = format!(
            "oculus-{:x}-{:x}",
            std::process::id(),
            crate::clock::now_nanos()
        );
        let mut fields = vec![
            ("model", MODEL),
            ("response_format", "verbose_json"),
            ("timestamp_granularities[]", "segment"),
            ("temperature", "0"),
        ];
        if let Some(language) = &self.language {
            fields.push(("language", language.as_str()));
        }
        let body = multipart(
            &boundary,
            &fields,
            &File {
                field: "file",
                name: &format!("audio{}", audio::EXTENSION),
                content_type: audio::CONTENT_TYPE,
                bytes: &bytes,
            },
        );

        let response = self.post(&format!("multipart/form-data; boundary={boundary}"), &body)?;
        if response.status >= 400 {
            return Err(refusal(
                response.status,
                response.header("retry-after"),
                response.header("x-ratelimit-reset-requests"),
                &String::from_utf8_lossy(&response.body),
            ));
        }
        let text = String::from_utf8(response.body)
            .map_err(|e| EngineError::Failed(format!("Groq's reply could not be read: {e}")))?;
        parse_segments(&text)
    }
}

pub(super) struct File<'a> {
    pub field: &'a str,
    pub name: &'a str,
    pub content_type: &'a str,
    pub bytes: &'a [u8],
}

/// A `multipart/form-data` body: text fields, then one file. ureq 2 has no
/// multipart support, and this is all of it Groq needs.
pub(super) fn multipart(boundary: &str, fields: &[(&str, &str)], file: &File) -> Vec<u8> {
    let mut body = Vec::with_capacity(file.bytes.len() + 1024);
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n\
             Content-Type: {}\r\n\r\n",
            file.field, file.name, file.content_type
        )
        .as_bytes(),
    );
    body.extend_from_slice(file.bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

/// `verbose_json`'s segments. A chunk of silence has none, which is not an
/// error here; the whole run having none is (`super::run`).
pub(super) fn parse_segments(body: &str) -> Result<Vec<Segment>, EngineError> {
    #[derive(serde::Deserialize)]
    struct Verbose {
        #[serde(default)]
        segments: Vec<Raw>,
    }
    #[derive(serde::Deserialize)]
    struct Raw {
        start: f64,
        end: f64,
        #[serde(default)]
        text: String,
    }
    let verbose: Verbose = serde_json::from_str(body)
        .map_err(|e| EngineError::Failed(format!("Groq's reply was not a transcript: {e}")))?;
    Ok(verbose
        .segments
        .into_iter()
        .map(|r| Segment {
            start: r.start,
            end: r.end,
            text: r.text,
        })
        .collect())
}

/// A refused request in words. A rate limit says when to come back: the
/// `retry-after` header, else Groq's own "try again in", else the reset header.
fn refusal(status: u16, retry_after: Option<&str>, reset: Option<&str>, body: &str) -> EngineError {
    let message = crate::groq::message_of(body);
    match status {
        429 => {
            let when = retry_after
                .and_then(|s| s.trim().parse::<u64>().ok())
                .map(in_words)
                .or_else(|| message.as_deref().and_then(try_again_in))
                .or_else(|| reset.map(|r| r.trim().to_string()))
                .map(|when| format!("try again in {when}"))
                .unwrap_or_else(|| "try again later".into());
            EngineError::RateLimited(format!("Groq's free-tier limit was hit — {when}"))
        }
        401 | 403 => EngineError::Failed(
            "Groq rejected the saved key — replace it in Settings → Transcription".into(),
        ),
        413 => EngineError::Failed("Groq refused the audio as too large to upload".into()),
        _ => EngineError::Failed(match message {
            Some(message) => format!("Groq could not transcribe this video ({status}): {message}"),
            None => format!("Groq could not transcribe this video ({status})"),
        }),
    }
}

/// `"... Please try again in 7m12.5s. Need more ..."` → `"7m12.5s"`.
fn try_again_in(message: &str) -> Option<String> {
    let rest = message.split("try again in ").nth(1)?;
    let when: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '.')
        .collect();
    let when = when.trim_end_matches('.');
    (!when.is_empty()).then(|| when.to_string())
}

fn in_words(seconds: u64) -> String {
    match seconds {
        0..=89 => format!("{seconds} seconds"),
        90..=5399 => format!("{} minutes", seconds.div_ceil(60)),
        _ => format!("{} hours", seconds.div_ceil(3600)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeServer, Reply, Scratch};
    use serde_json::json;

    #[test]
    fn the_multipart_body_has_each_field_then_the_file() {
        let body = multipart(
            "B",
            &[("model", "whisper-large-v3-turbo"), ("temperature", "0")],
            &File {
                field: "file",
                name: "audio.ogg",
                content_type: "audio/ogg",
                bytes: b"\x00OggS\xff",
            },
        );
        let mut expected = b"--B\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n\
whisper-large-v3-turbo\r\n\
--B\r\nContent-Disposition: form-data; name=\"temperature\"\r\n\r\n0\r\n\
--B\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.ogg\"\r\n\
Content-Type: audio/ogg\r\n\r\n"
            .to_vec();
        expected.extend_from_slice(b"\x00OggS\xff");
        expected.extend_from_slice(b"\r\n--B--\r\n");
        assert_eq!(body, expected);
    }

    #[test]
    fn verbose_json_segments_are_read_and_extra_fields_ignored() {
        let segments = parse_segments(
            &json!({
                "task": "transcribe",
                "language": "English",
                "duration": 12.0,
                "text": " Hello. World.",
                "segments": [
                    {"id": 0, "seek": 0, "start": 0.0, "end": 4.5, "text": " Hello.",
                     "tokens": [1, 2], "avg_logprob": -0.2, "no_speech_prob": 0.01},
                    {"id": 1, "seek": 0, "start": 4.5, "end": 12.0, "text": " World."}
                ],
                "x_groq": {"id": "req_1"}
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(
            segments[1],
            Segment {
                start: 4.5,
                end: 12.0,
                text: " World.".into()
            }
        );
        assert!(parse_segments(r#"{"text":""}"#).unwrap().is_empty());
        assert!(matches!(
            parse_segments("<html>"),
            Err(EngineError::Failed(_))
        ));
    }

    #[test]
    fn a_rate_limit_says_when_to_come_back() {
        let limit = |retry, reset, body: &str| match refusal(429, retry, reset, body) {
            EngineError::RateLimited(message) => message,
            other => panic!("expected a rate limit, got {other:?}"),
        };
        assert!(limit(Some("42"), None, "").ends_with("try again in 42 seconds"));
        assert!(limit(Some("600"), None, "").ends_with("try again in 10 minutes"));
        let body = json!({"error": {"message":
            "Rate limit reached for model `whisper-large-v3-turbo` on seconds of audio per hour \
             (ASPH): Limit 7200, Used 7166, Requested 3536. Please try again in 26m27.5s. \
             Need more tokens? Upgrade."}})
        .to_string();
        assert!(limit(None, None, &body).ends_with("try again in 26m27.5s"));
        assert!(limit(None, Some("2m59.56s"), "").ends_with("try again in 2m59.56s"));
        assert!(limit(None, None, "").ends_with("try again later"));
    }

    #[test]
    fn a_refused_key_points_at_settings_and_other_errors_carry_groqs_message() {
        assert!(matches!(
            refusal(401, None, None, r#"{"error":{"message":"Invalid API Key"}}"#),
            EngineError::Failed(m) if m.contains("Settings")
        ));
        assert!(matches!(
            refusal(400, None, None, r#"{"error":{"message":"file must be one of ..."}}"#),
            EngineError::Failed(m) if m.contains("(400): file must be one of")
        ));
    }

    #[test]
    fn a_transcription_posts_the_audio_and_reads_the_segments_back() {
        let server = FakeServer::start(|_| {
            Reply::json(json!({"segments": [{"start": 1.0, "end": 2.0, "text": " Hi."}]}))
        });
        let dir = Scratch::new("groq-client");
        let audio = dir.join("part.ogg");
        std::fs::write(&audio, b"OggS-audio").unwrap();

        let groq = Groq {
            auth: Auth::Direct("gsk_test".into()),
            base_url: server.origin(),
            language: Some("en".into()),
        };
        let segments = groq.transcribe(&audio).unwrap();
        assert_eq!(
            segments,
            vec![Segment {
                start: 1.0,
                end: 2.0,
                text: " Hi.".into()
            }]
        );

        let hit = &server.hits()[0];
        assert_eq!(hit.method, "POST");
        assert_eq!(hit.url, "/audio/transcriptions");
        assert_eq!(hit.header("authorization"), Some("Bearer gsk_test"));
        assert!(hit
            .header("content-type")
            .unwrap()
            .starts_with("multipart/form-data; boundary="));
        let body = String::from_utf8_lossy(&hit.body);
        assert!(body.contains("name=\"response_format\"\r\n\r\nverbose_json\r\n"));
        assert!(body.contains("name=\"timestamp_granularities[]\"\r\n\r\nsegment\r\n"));
        assert!(body.contains("name=\"language\"\r\n\r\nen\r\n"));
        assert!(body
            .contains("filename=\"audio.ogg\"\r\nContent-Type: audio/ogg\r\n\r\nOggS-audio\r\n"));
    }

    #[test]
    fn a_429_from_the_wire_is_a_rate_limit_with_its_retry_after() {
        let server = FakeServer::start(|_| {
            Reply::status(429, json!({"error": {"message": "Rate limit reached"}}))
                .with_header("retry-after", "120")
        });
        let dir = Scratch::new("groq-429");
        let audio = dir.join("part.ogg");
        std::fs::write(&audio, b"OggS").unwrap();

        let groq = Groq {
            auth: Auth::Direct("gsk_test".into()),
            base_url: server.origin(),
            language: None,
        };
        match groq.transcribe(&audio) {
            Err(EngineError::RateLimited(message)) => assert!(message.contains("2 minutes")),
            other => panic!("expected a rate limit, got {other:?}"),
        }
    }

    // ── Through oculus-keyd ──────────────────────────────────────────────────

    /// A Groq client through a fake keyd that answers each `forward` with
    /// `answer`, and the audio file it uploads.
    fn through_keyd<F>(answer: F) -> (Groq, crate::test_support::FakeKeyd, Scratch)
    where
        F: Fn() -> (serde_json::Value, Vec<u8>) + Send + 'static,
    {
        let dir = Scratch::new("groq-keyd");
        std::fs::write(dir.join("part.ogg"), b"OggS-audio").unwrap();
        let keyd = crate::test_support::FakeKeyd::start(&dir, move |_, _| answer());
        let groq = Groq::new(Auth::Keyd(Credentialed::at(&dir)), Some("en".into()));
        (groq, keyd, dir)
    }

    fn forwarded(
        status: u16,
        headers: serde_json::Value,
        body: &serde_json::Value,
    ) -> (serde_json::Value, Vec<u8>) {
        let body = body.to_string().into_bytes();
        (
            json!({ "status": status, "headers": headers, "body_len": body.len() }),
            body,
        )
    }

    #[test]
    fn through_keyd_the_audio_is_forwarded_without_the_key() {
        let (groq, keyd, dir) = through_keyd(|| {
            forwarded(
                200,
                json!([]),
                &json!({"segments": [{"start": 1.0, "end": 2.0, "text": " Hi."}]}),
            )
        });
        let segments = groq.transcribe(&dir.join("part.ogg")).unwrap();
        assert_eq!(segments.len(), 1);

        let (header, body) = &keyd.requests()[0];
        assert_eq!(header["op"], "forward");
        assert_eq!(header["secret"], "groq");
        assert_eq!(header["method"], "POST");
        assert_eq!(header["path"], "/openai/v1/audio/transcriptions");
        let headers = header["headers"].as_array().unwrap();
        assert_eq!(headers.len(), 1, "{header}");
        assert_eq!(headers[0][0], "Content-Type");
        assert!(headers[0][1]
            .as_str()
            .unwrap()
            .starts_with("multipart/form-data; boundary=oculus-"));
        assert!(
            !header.to_string().to_lowercase().contains("authorization"),
            "{header}"
        );
        let body = String::from_utf8_lossy(body);
        assert!(body.contains("name=\"language\"\r\n\r\nen\r\n"));
        assert!(body.contains("OggS-audio"));
    }

    #[test]
    fn through_keyd_refusals_read_as_they_do_direct() {
        let (groq, _keyd, dir) = through_keyd(|| {
            forwarded(
                429,
                json!([["retry-after", "120"]]),
                &json!({"error": {"message": "Rate limit reached"}}),
            )
        });
        match groq.transcribe(&dir.join("part.ogg")) {
            Err(EngineError::RateLimited(message)) => assert!(message.contains("2 minutes")),
            other => panic!("expected a rate limit, got {other:?}"),
        }

        let (groq, _keyd, dir) = through_keyd(|| {
            forwarded(
                401,
                json!([]),
                &json!({"error": {"message": "Invalid API Key"}}),
            )
        });
        assert!(matches!(
            groq.transcribe(&dir.join("part.ogg")),
            Err(EngineError::Failed(m)) if m.starts_with("Groq rejected the saved key")
        ));
    }

    #[test]
    fn keyds_own_errors_say_what_failed() {
        for (wire, start) in [
            ("missing", "No Groq API key"),
            (
                "keychain",
                "The keychain refused to give out the Groq API key (refused)",
            ),
            ("upstream", "could not reach Groq: refused"),
            (
                "vault",
                "oculus-keyd, which holds the Groq API key, could not send this request",
            ),
        ] {
            let (groq, _keyd, dir) =
                through_keyd(move || (json!({"error": wire, "detail": "refused"}), vec![]));
            match groq.transcribe(&dir.join("part.ogg")) {
                Err(EngineError::Failed(message)) => {
                    assert!(message.starts_with(start), "{wire}: {message}")
                }
                other => panic!("{wire}: expected a failure, got {other:?}"),
            }
        }
    }
}

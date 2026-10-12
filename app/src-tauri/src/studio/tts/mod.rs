//! The narration TTS client: one `POST {base}/v1/audio/speech` per clip to
//! an OpenAI-compatible server (Kokoro-FastAPI, mlx-audio), asking for WAV.
//! Keyless only. No timeout: a cold server takes many seconds to load its
//! model. A clip that is empty, not a WAV or digitally silent is a failure,
//! never a pause (Kokoro answers an unknown voice with 200 and no audio).
//! `models` lists the server's model ids for Settings -> Studio -> Narration.

pub mod kokoro;
mod models;
#[cfg(test)]
mod tests;

use std::io::Read;

use serde_json::json;

use super::audio::{decode_wav, silence, Decoded};
use super::voice::ServerVoice;
use crate::providers::ratelimit::{transport_detail, Retry};
pub use models::list_models;

/// Tries per clip when the server answers 429, backing off 1 s, 2 s, 4 s...
pub const ATTEMPTS: u32 = 5;

/// A clip as the server sent it, and decoded.
pub struct Speech {
    /// The response body, byte for byte: a WAV.
    pub wav: Vec<u8>,
    pub decoded: Decoded,
}

/// `url` as a base the client appends `/v1/audio/speech` to: trimmed, with
/// trailing slashes and one trailing `/v1` removed. Only http and https.
pub fn normalise_base(url: &str) -> Result<String, String> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(format!(
            "TTS server `{url}` must be an http:// or https:// URL, e.g. http://127.0.0.1:8880"
        ));
    }
    let mut base = url.trim_end_matches('/');
    if let Some(stripped) = base.strip_suffix("/v1") {
        base = stripped.trim_end_matches('/');
    }
    if base
        .split_once("://")
        .is_none_or(|(_, host)| host.is_empty())
    {
        return Err(format!("TTS server `{url}` has no host"));
    }
    Ok(base.to_owned())
}

/// One clip of `text` in `voice`.
pub fn synthesise(voice: &ServerVoice, text: &str) -> Result<Speech, String> {
    synthesise_scaled(voice, text, 1.0)
}

/// [`synthesise`] with the 429 back-off's sleeps scaled by `time_scale`
/// (tests pass ~0).
pub(crate) fn synthesise_scaled(
    voice: &ServerVoice,
    text: &str,
    time_scale: f64,
) -> Result<Speech, String> {
    let url = format!("{}/v1/audio/speech", voice.base_url);
    let body = json!({
        "model": voice.model,
        "input": text,
        "voice": voice.voice,
        "response_format": "wav",
        "speed": voice.speed,
    })
    .to_string();
    let agent = ureq::AgentBuilder::new().build();
    let mut retry = Retry::new(ATTEMPTS, time_scale);
    let response = loop {
        let request = agent.post(&url).set("Content-Type", "application/json");
        match request.send_string(&body) {
            Ok(response) => break response,
            Err(ureq::Error::Status(429, response)) => {
                if !retry.back_off() {
                    return Err(format!(
                        "the TTS server at {} is rate-limiting (429) after {ATTEMPTS} tries: {}",
                        voice.base_url,
                        excerpt(&read_body(response))
                    ));
                }
            }
            Err(ureq::Error::Status(status, response)) => {
                return Err(format!(
                    "the TTS server answered {status} for model `{}`, voice `{}`: {}",
                    voice.model,
                    voice.voice,
                    excerpt(&read_body(response))
                ));
            }
            Err(ureq::Error::Transport(t)) => {
                return Err(format!(
                    "could not reach the TTS server at {}: {}",
                    voice.base_url,
                    transport_detail(&t)
                ));
            }
        }
    };
    let status = response.status();
    let wav = read_body(response);
    check(&wav, status, voice).map(|decoded| Speech { wav, decoded })
}

/// The clip, decoded, or why it is not narration.
fn check(wav: &[u8], status: u16, voice: &ServerVoice) -> Result<Decoded, String> {
    if wav.is_empty() {
        return Err(format!(
            "the TTS server answered {status} with an empty body for voice `{}`; check the voice name (Kokoro answers an unknown voice this way)",
            voice.voice
        ));
    }
    let decoded = decode_wav(wav).map_err(|e| {
        format!(
            "the TTS server answered {status} with a body that is not a WAV ({e}): {}",
            excerpt(wav)
        )
    })?;
    if let Some(s) = silence(&decoded.samples, decoded.sample_rate) {
        return Err(format!(
            "the TTS server answered {status} with a digitally silent WAV ({s}) for voice `{}`; check the voice name",
            voice.voice
        ));
    }
    Ok(decoded)
}

/// The whole body; a read error leaves what arrived.
fn read_body(response: ureq::Response) -> Vec<u8> {
    let mut body = Vec::new();
    response.into_reader().read_to_end(&mut body).ok();
    body
}

/// A short, one-line excerpt of a body for an error message.
fn excerpt(body: &[u8]) -> String {
    const MAX: usize = 200;
    if body.is_empty() {
        return "(empty body)".into();
    }
    let text = String::from_utf8_lossy(body);
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= MAX {
        return line;
    }
    format!("{}…", line.chars().take(MAX).collect::<String>())
}

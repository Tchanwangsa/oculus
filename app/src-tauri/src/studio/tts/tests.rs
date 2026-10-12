use serde_json::json;

use super::*;
use crate::studio::test_wav::{tone, wav_of};
use crate::test_support::{FakeServer, Reply};

fn voice(base: &str) -> ServerVoice {
    ServerVoice {
        base_url: normalise_base(base).unwrap(),
        model: "kokoro".into(),
        voice: "af_heart".into(),
        speed: 1.1,
    }
}

#[test]
fn the_base_url_takes_or_leaves_v1_and_slashes() {
    for url in [
        "http://127.0.0.1:8880",
        "http://127.0.0.1:8880/",
        "http://127.0.0.1:8880/v1",
        "http://127.0.0.1:8880/v1/",
        "  http://127.0.0.1:8880/v1//  ",
    ] {
        assert_eq!(
            normalise_base(url).unwrap(),
            "http://127.0.0.1:8880",
            "{url}"
        );
    }
    assert_eq!(
        normalise_base("https://tts.example/api/v1").unwrap(),
        "https://tts.example/api"
    );
    assert_eq!(
        normalise_base("https://tts.example/v1x").unwrap(),
        "https://tts.example/v1x"
    );
    assert!(normalise_base("127.0.0.1:8880").is_err());
    assert!(normalise_base("http://").is_err());
    assert!(normalise_base("http:///v1").is_err());
}

#[test]
fn a_wav_reply_is_the_clip_and_the_request_is_openai_shaped() {
    let clip = tone(0.5);
    let body = clip.clone();
    let server = FakeServer::start(move |_| Reply::bytes(body.clone()));
    let speech = synthesise(&voice(&format!("{}/v1", server.origin())), "Hello there.").unwrap();
    assert_eq!(speech.wav, clip);
    assert_eq!(speech.decoded.sample_rate, 24_000);
    assert!((speech.decoded.duration - 0.5).abs() < 1e-9);
    let hits = server.hits();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        (hits[0].method.as_str(), hits[0].url.as_str()),
        ("POST", "/v1/audio/speech")
    );
    assert_eq!(
        hits[0].json(),
        json!({ "model": "kokoro", "input": "Hello there.", "voice": "af_heart",
            "response_format": "wav", "speed": 1.1 })
    );
    assert!(hits[0].header("authorization").is_none(), "keyless");
}

#[test]
fn an_empty_body_is_a_failure_naming_the_voice() {
    let server = FakeServer::start(|_| Reply::bytes(Vec::new()));
    let e = synthesise(&voice(&server.origin()), "Hi.").err().unwrap();
    assert!(e.contains("empty body") && e.contains("af_heart"), "{e}");
}

#[test]
fn a_body_that_is_not_a_wav_is_a_failure_with_an_excerpt() {
    let server = FakeServer::start(|_| Reply::bytes(b"ID3 not a wave at all".to_vec()));
    let e = synthesise(&voice(&server.origin()), "Hi.").err().unwrap();
    assert!(
        e.contains("not a WAV") && e.contains("ID3 not a wave"),
        "{e}"
    );
}

#[test]
fn an_all_zero_wav_is_silent_and_fails() {
    let server = FakeServer::start(|_| Reply::bytes(wav_of(24_000, 2.0, |_| 0)));
    let e = synthesise(&voice(&server.origin()), "Hi.").err().unwrap();
    assert!(e.contains("digitally silent"), "{e}");
}

#[test]
fn a_404_reports_its_status_and_body() {
    let server = FakeServer::start(|_| {
        Reply::status(404, json!({ "detail": "Model nonexistent not found" }))
    });
    let e = synthesise(&voice(&server.origin()), "Hi.").err().unwrap();
    assert!(
        e.contains("404") && e.contains("Model nonexistent not found") && e.contains("kokoro"),
        "{e}"
    );
    assert_eq!(server.hits().len(), 1, "no retry on a 404");
}

#[test]
fn a_429_backs_off_then_succeeds() {
    let clip = tone(0.3);
    let server = FakeServer::start(move |hit| match hit.index {
        0 | 1 => Reply::status(429, json!({ "error": "slow down" })),
        _ => Reply::bytes(clip.clone()),
    });
    let speech = synthesise_scaled(&voice(&server.origin()), "Hi.", 0.0).unwrap();
    assert!((speech.decoded.duration - 0.3).abs() < 1e-9);
    assert_eq!(server.hits().len(), 3);
}

#[test]
fn a_429_that_never_ends_gives_up_after_the_attempts() {
    let server = FakeServer::start(|_| Reply::status(429, json!({ "error": "slow down" })));
    let e = synthesise_scaled(&voice(&server.origin()), "Hi.", 0.0)
        .err()
        .unwrap();
    assert!(e.contains("429") && e.contains("slow down"), "{e}");
    assert_eq!(server.hits().len(), ATTEMPTS as usize);
}

#[test]
fn an_unreachable_server_names_its_url() {
    let base = crate::test_support::dead_origin();
    let e = synthesise(&voice(&base), "Hi.").err().unwrap();
    assert!(e.contains("could not reach") && e.contains(&base), "{e}");
}

#[test]
fn the_model_listing_reads_data_ids_in_order_without_repeats() {
    let body = br#"{"object":"list","data":[{"id":"kokoro"},{"id":" tts-1 "},{"id":"kokoro"},{"name":"x"},{"id":""},{"id":7}]}"#;
    assert_eq!(models::parse_models(body).unwrap(), ["kokoro", "tts-1"]);
    assert_eq!(
        models::parse_models(br#"{"data":[]}"#).unwrap(),
        Vec::<String>::new()
    );
    assert!(models::parse_models(b"<html>").is_err());
    assert!(models::parse_models(br#"{"models":[]}"#).is_err());
}

#[test]
fn the_model_listing_gets_v1_models_from_the_normalised_base() {
    let server = FakeServer::start(|hit| match (hit.method.as_str(), hit.url.as_str()) {
        ("GET", "/v1/models") => Reply::json(json!({
            "data": [{ "id": "mlx-community/Kokoro-82M-bf16" }, { "id": "kokoro" }]
        })),
        _ => Reply::status(404, json!({})),
    });
    let ids = list_models(&format!("{}/v1/", server.origin())).unwrap();
    assert_eq!(ids, ["mlx-community/Kokoro-82M-bf16", "kokoro"]);
    let hits = server.hits();
    assert_eq!(hits.len(), 1, "one GET, never a synthesis");
    assert_eq!(hits[0].method, "GET");
}

#[test]
fn the_model_listing_fails_with_the_servers_answer() {
    let server = FakeServer::start(|_| Reply::status(404, json!({ "detail": "Not Found" })));
    let e = list_models(&server.origin()).unwrap_err();
    assert!(e.contains("404") && e.contains("type the model id"), "{e}");

    let server = FakeServer::start(|_| Reply::bytes(b"not json".to_vec()));
    let e = list_models(&server.origin()).unwrap_err();
    assert!(e.contains("not JSON") && e.contains("not json"), "{e}");

    let e = list_models(&crate::test_support::dead_origin()).unwrap_err();
    assert!(e.contains("could not reach"), "{e}");
    assert!(list_models("localhost:8880").is_err());
}

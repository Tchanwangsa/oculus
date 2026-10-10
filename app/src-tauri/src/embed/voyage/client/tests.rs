use super::send::ATTEMPTS;
use super::wire::{decode_embedding, decode_response};
use super::*;
use crate::embed::raster::RenderedPage;
use crate::embed::voyage::batch;
use crate::embed::voyage::batch::Limits;
use crate::embed::voyage::batch::RequestRun;
use crate::embed::voyage::ledger::{RateGate, TierSource, UsageLedger, FREE_TPM};
use crate::embed::Embedder;
use crate::embed::{EmbedError, Limiter, Wait, EMBED_DIM, EMBED_MODEL};
use crate::providers::ratelimit::hold;
use crate::test_support::{write_pdf, write_pdf_sized, FakeServer, Reply, Scratch};
use base64::Engine as _;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

/// Landscape A4, 842 x 595 pt: over the billed-pixel cap at `RENDER_DPI`.
fn write_a4_pdf(path: &Path, pages: usize) {
    write_pdf_sized(path, pages, 842, 595);
}

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
    let fake =
        FakeServer::start(|_| Reply::status(403, json!({ "detail": "This API key has expired." })));
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

#[test]
fn a_document_embeds_every_page_and_reports_real_progress() {
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
    let fake = FakeServer::start(|_| ok_response(1));
    let scratch = Scratch::new("voyage-count");
    let pdf = scratch.join("deck.pdf");
    write_pdf(&pdf, 3);

    // The parse record says four pages; the file has three.
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
fn health_names_the_space_and_is_ready_with_a_key() {
    let fake = FakeServer::start(|_| ok_response(1));
    let scratch = Scratch::new("voyage-health");
    let health = client(&fake, &scratch).health();
    assert_eq!(health.model, EMBED_MODEL);
    assert_eq!(health.dim, EMBED_DIM);
    assert!(health.ready);
    health.check().unwrap();
}

#[test]
fn a_missing_key_is_refused_before_a_client_exists() {
    assert!(matches!(
        VoyageCloud::new("https://example.invalid/v1", "   "),
        Err(EmbedError::MissingCredentials)
    ));
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

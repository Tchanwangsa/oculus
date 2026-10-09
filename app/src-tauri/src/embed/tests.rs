use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::test_support::Scratch;

/// `embed_settings` is an async command; a `block_on` on its worker thread
/// panics. Surviving the call is what is tested, so it passes without a DB.
#[test]
fn the_config_is_readable_from_inside_an_async_runtime() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let base = runtime.block_on(async { embed_config().base_url });
    assert!(
        !base.is_empty(),
        "a backend always resolves to some API root"
    );
}

/// A real scratch directory: the record's temp+rename must be same-filesystem.
fn scratch(name: &str) -> Scratch {
    Scratch::new(&format!("embed-{name}"))
}

fn sample_pdf(dir: &Path) -> PathBuf {
    let pdf = dir.join("Lecture 3.pdf");
    fs::write(&pdf, b"%PDF-1.4").unwrap();
    pdf
}

/// Deliberately not unit length, so every round trip also tests normalising.
fn raw_vector(seed: u32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| ((i as u32 * 37 + seed) % 101) as f32 - 50.0)
        .collect()
}

/// A tolerance, not an equality, because of f16 rounding.
fn assert_unit_norm(vector: &[f32]) {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 2e-3, "‖v‖ = {norm}, not 1");
}

#[test]
fn artifact_name_matches_the_records_already_on_disk() {
    let pdf = Path::new("/library/subj/Lecture 3.pdf");
    assert_eq!(emb_path(pdf), Path::new("/library/subj/Lecture 3.emb.json"));
}

#[test]
fn f16_round_trips_through_base64_at_the_stored_width() {
    let raw = raw_vector(7, EMBED_DIM);
    let encoded = encode_vector(&raw).unwrap();
    let bytes = pack_vector(&raw).unwrap();

    // The blob column's width.
    assert_eq!(bytes.len(), EMBED_DIM * 2);

    let decoded = decode_vector(&encoded).unwrap();
    assert_eq!(decoded.len(), EMBED_DIM);
    assert_eq!(decoded, unpack_vector(&bytes));

    // Same direction as what went in, at f16 precision, after normalising.
    let norm = raw
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        .sqrt();
    for (i, value) in decoded.iter().enumerate() {
        let expected = (raw[i] as f64 / norm) as f32;
        assert!(
            (value - expected).abs() < 1e-3,
            "dim {i}: {value} vs {expected}"
        );
    }
}

#[test]
fn vectors_are_stored_normalised() {
    let raw = raw_vector(3, EMBED_DIM);
    assert_unit_norm(&decode_vector(&encode_vector(&raw).unwrap()).unwrap());

    // Matryoshka: a longer vector is truncated to EMBED_DIM and
    // *re-normalised*, because a prefix of a unit vector is not one.
    let long = raw_vector(11, EMBED_DIM * 2);
    let page = EmbedPage::new(1, &long).unwrap();
    let stored = decode_vector(&page.vector).unwrap();
    assert_eq!(stored.len(), EMBED_DIM);
    assert_unit_norm(&stored);

    let self_score: f32 = stored.iter().map(|v| v * v).sum();
    assert!((self_score - 1.0).abs() < 2e-3, "{self_score}");
}

#[test]
fn a_short_vector_is_a_different_space_not_something_to_pad() {
    let error = pack_vector(&raw_vector(1, EMBED_DIM - 1)).unwrap_err();
    assert!(
        matches!(error, EmbedError::ModelMismatch { backend_dim, .. } if backend_dim == EMBED_DIM - 1)
    );
    assert!(!error.retryable());
    assert!(error.latching());
}

#[test]
fn a_zero_vector_is_refused_rather_than_stored_unrankable() {
    let error = pack_vector(&vec![0.0; EMBED_DIM]).unwrap_err();
    assert_eq!(error.kind(), "document");
    // Scoped to the file: the rest of the queue keeps going.
    assert!(!error.latching());
}

#[test]
fn record_keeps_its_wire_shape() {
    let pdf = Path::new("/library/Lecture 3.pdf");
    let out = EmbedOutput::new(
        pdf,
        1,
        vec![EmbedPage::new(1, &raw_vector(5, EMBED_DIM)).unwrap()],
    );
    let json = serde_json::to_string(&out).unwrap();
    for key in [
        "pdf",
        "model",
        "dim",
        "dtype",
        "instruction",
        "page_count",
        "pages",
        "page_no",
        "vector",
    ] {
        assert!(
            json.contains(&format!("\"{key}\"")),
            "missing {key}: {json}"
        );
    }
    assert!(json.contains("\"dim\":512"), "{json}");
    assert!(json.contains("\"dtype\":\"float16\""), "{json}");
    // And it reads back as the type that wrote it.
    let back: EmbedOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(back.model, EMBED_MODEL);
    assert_eq!(back.pages[0].vector, out.pages[0].vector);
}

#[test]
fn a_record_from_another_model_deserialises_and_re_embeds() {
    let dir = scratch("legacy");
    let pdf = sample_pdf(&dir);
    fs::write(
        emb_path(&pdf),
        r#"{"pdf":"Lecture 3.pdf","model":"Qwen/Qwen3-VL-Embedding-2B","dim":512,
            "dtype":"float16","instruction":"Given a student's question, retrieve the lecture
            slide that answers it.","page_count":1,"pages":[{"page_no":1,"vector":"AAA="}]}"#
            .replace('\n', ""),
    )
    .unwrap();

    let record = read_record(&pdf).expect("legacy record should parse");
    assert_eq!(record.dim, EMBED_DIM);
    assert!(!is_embedded(&pdf));
}

#[test]
fn pages_are_ordered_and_missing_ones_are_dropped_not_filled() {
    let pdf = Path::new("/library/Lecture 3.pdf");
    let out = EmbedOutput::new(
        pdf,
        4,
        vec![
            EmbedPage::new(3, &raw_vector(3, EMBED_DIM)).unwrap(),
            EmbedPage::new(1, &raw_vector(1, EMBED_DIM)).unwrap(),
            // Out of range.
            EmbedPage::new(9, &raw_vector(9, EMBED_DIM)).unwrap(),
            // Duplicate page number.
            EmbedPage::new(1, &raw_vector(2, EMBED_DIM)).unwrap(),
        ],
    );
    assert_eq!(
        out.pages.iter().map(|p| p.page_no).collect::<Vec<_>>(),
        vec![1, 3]
    );
    // Counts embedded pages, not document pages.
    assert_eq!(out.page_count, 2);
}

#[test]
fn write_lands_the_record_atomically_and_leaves_no_temp() {
    let dir = scratch("write");
    let pdf = sample_pdf(&dir);

    let out = EmbedOutput::new(
        &pdf,
        2,
        vec![
            EmbedPage::new(1, &raw_vector(1, EMBED_DIM)).unwrap(),
            EmbedPage::new(2, &raw_vector(2, EMBED_DIM)).unwrap(),
        ],
    );
    out.write(&pdf).unwrap();

    assert!(is_embedded(&pdf));
    assert_eq!(read_record(&pdf).unwrap().page_count, 2);
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn a_different_space_is_refused_and_names_both_sides() {
    let health = Health {
        backend: "oculus-local".into(),
        model: "some-other-embedder".into(),
        dim: 768,
        ready: true,
    };
    let error = health.check().unwrap_err();
    assert_eq!(error.kind(), "model_mismatch");
    let shown = error.to_string();
    assert!(shown.contains("some-other-embedder"), "{shown}");
    assert!(shown.contains(EMBED_MODEL), "{shown}");
    assert!(shown.contains("768") && shown.contains("512"), "{shown}");
    assert!(!error.retryable());
    assert!(error.latching());

    let waiting = Health {
        backend: "voyage".into(),
        model: EMBED_MODEL.into(),
        dim: EMBED_DIM,
        ready: false,
    };
    let error = waiting.check().unwrap_err();
    assert_eq!(error.kind(), "not_ready");
    assert!(error.retryable());
    assert!(!error.latching());

    let ok = Health {
        backend: "voyage".into(),
        model: EMBED_MODEL.into(),
        dim: EMBED_DIM,
        ready: true,
    };
    assert!(ok.check().is_ok());
}

#[test]
fn a_rate_limit_is_routine_not_fatal() {
    let error = EmbedError::RateLimited {
        retry_after_secs: Some(20),
    };
    assert_eq!(error.kind(), "rate_limited");
    assert!(error.retryable());
    assert!(!error.latching());
    assert!(EmbedError::QuotaExhausted.retryable());
    assert!(EmbedError::QuotaExhausted.latching());
}

#[test]
fn credential_failures_keep_the_word_the_failure_ui_matches_on() {
    for error in [
        EmbedError::MissingCredentials,
        EmbedError::RejectedCredentials {
            code: None,
            expired: false,
        },
    ] {
        assert!(error.kind().contains("credential"), "{}", error.kind());
        assert!(!error.retryable());
        assert!(error.latching());
    }
    // Retrying asks the keychain again, and that prompt can be allowed.
    let unreadable = EmbedError::UnreadableCredentials("denied".into());
    assert!(
        unreadable.kind().contains("credential"),
        "{}",
        unreadable.kind()
    );
    assert!(unreadable.retryable());
    assert!(unreadable.latching());
    assert!(
        !unreadable.to_string().contains("No Voyage API key"),
        "{unreadable}"
    );
    let shown = EmbedError::RejectedCredentials {
        code: Some("401".into()),
        expired: true,
    }
    .to_string();
    assert!(shown.contains("Settings"), "{shown}");
    assert!(shown.contains("401"), "{shown}");
}

#[test]
fn a_stale_fallback_policy_is_not_an_engine() {
    assert_eq!(Engine::parse("auto"), None);
    assert_eq!(Engine::parse(""), None);
    assert_eq!(Engine::parse("cloud"), Some(Engine::Cloud));
    assert_eq!(Engine::parse(" local "), Some(Engine::Local));
    assert_eq!(Engine::Cloud.as_str(), "cloud");
}

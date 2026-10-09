use super::download::*;
use super::models::*;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use crate::test_support::{FakeServer, Reply, Scratch};

const GB: u64 = 1 << 30;

fn fit_of(total: u64, gpu: bool) -> Vec<(&'static str, Fit)> {
    MODELS.iter().map(|m| m.id).zip(fits(total, gpu)).collect()
}

fn recommended(total: u64, gpu: bool) -> &'static str {
    fit_of(total, gpu)
        .into_iter()
        .find(|(_, f)| *f == Fit::Recommended)
        .unwrap()
        .0
}

fn too_large(total: u64, gpu: bool) -> Vec<&'static str> {
    fit_of(total, gpu)
        .into_iter()
        .filter(|(_, f)| *f == Fit::TooLarge)
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn exactly_one_model_is_recommended_on_any_machine() {
    for total in [GB, 2 * GB, 4 * GB, 8 * GB, 16 * GB, 36 * GB, 128 * GB] {
        for gpu in [true, false] {
            let fits = fits(total, gpu);
            assert_eq!(fits.len(), MODELS.len());
            assert_eq!(
                fits.iter().filter(|f| **f == Fit::Recommended).count(),
                1,
                "{total} {gpu}"
            );
        }
    }
}

#[test]
fn apple_silicon_macs_of_8_16_and_36_gb_get_the_quantised_turbo() {
    for total in [8 * GB, 16 * GB, 36 * GB] {
        assert_eq!(recommended(total, true), DEFAULT_MODEL);
        assert!(too_large(total, true).is_empty(), "{total}");
    }
}

#[test]
fn small_machines_fall_back_and_mark_what_will_not_fit() {
    assert_eq!(recommended(2 * GB, true), "base");
    assert_eq!(
        too_large(2 * GB, true),
        vec!["medium", "large-v3-turbo", "large-v3"]
    );
    assert_eq!(too_large(4 * GB, true), vec!["large-v3"]);
    assert_eq!(recommended(GB / 2, true), "tiny");
}

#[test]
fn without_a_gpu_the_recommendation_stops_at_small() {
    assert_eq!(recommended(8 * GB, false), "small");
    assert_eq!(recommended(36 * GB, false), "small");
    assert_eq!(recommended(2 * GB, false), "base");
}

#[test]
fn the_catalogue_names_ggml_files_on_hugging_face() {
    for m in &MODELS {
        assert_eq!(m.file, format!("ggml-{}.bin", m.id));
        assert_eq!(m.url(), format!("{MODEL_BASE}/ggml-{}.bin", m.id));
    }
    assert!(find(DEFAULT_MODEL).is_some());
}

#[test]
fn a_run_uses_the_chosen_model_else_the_default_else_the_best_on_disk() {
    let dir = Scratch::new("whisper-pick");
    assert!(pick(&dir, None)
        .unwrap_err()
        .contains("no Whisper model is downloaded"));
    std::fs::write(dir.join("ggml-tiny.bin"), b"x").unwrap();
    std::fs::write(dir.join("ggml-small.bin"), b"x").unwrap();
    assert_eq!(pick(&dir, None).unwrap(), dir.join("ggml-small.bin"));
    std::fs::write(dir.join("ggml-large-v3-turbo-q5_0.bin"), b"x").unwrap();
    assert_eq!(
        pick(&dir, None).unwrap(),
        dir.join("ggml-large-v3-turbo-q5_0.bin")
    );
    assert_eq!(pick(&dir, Some("tiny")).unwrap(), dir.join("ggml-tiny.bin"));
    assert!(pick(&dir, Some("medium"))
        .unwrap_err()
        .contains("Medium is not downloaded"));
    assert!(pick(&dir, Some("huge"))
        .unwrap_err()
        .contains("no Whisper model called huge"));
}

#[test]
fn the_listing_reads_the_directory_only() {
    let dir = Scratch::new("whisper-list");
    std::fs::write(dir.join("ggml-base.bin"), b"x").unwrap();
    std::fs::write(dir.join("ggml-small.bin.part"), b"x").unwrap();
    let listed = list(&dir);
    let downloaded: Vec<&str> = listed
        .models
        .iter()
        .filter(|m| m.downloaded)
        .map(|m| m.id)
        .collect();
    assert_eq!(downloaded, vec!["base"]);
    assert!(!listed.vad_downloaded);
    let json = serde_json::to_value(&listed).unwrap();
    assert!(json["models"][0].get("ramBytes").is_some());
    assert!(json["models"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["fit"] == "recommended"));
}

#[test]
fn a_download_fetches_the_vad_model_then_the_model_and_reports_progress() {
    let server = FakeServer::start(|hit| match hit.url.as_str() {
        "/vad.bin" => Reply::bytes(vec![1; 1000]),
        "/model.bin" => Reply::bytes(vec![2; 300_000]),
        _ => Reply::status(404, serde_json::json!({})),
    });
    let dir = Scratch::new("whisper-download");
    let model = find("tiny").unwrap();
    let seen = Mutex::new(Vec::new());
    let path = download_from(
        &dir,
        model,
        &format!("{}/model.bin", server.origin()),
        &format!("{}/vad.bin", server.origin()),
        |received, total| seen.lock().unwrap().push((received, total)),
    )
    .unwrap();
    assert_eq!(path, dir.join("ggml-tiny.bin"));
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 300_000);
    assert_eq!(std::fs::metadata(dir.join(VAD_FILE)).unwrap().len(), 1000);
    assert!(!dir.join("ggml-tiny.bin.part").exists());
    let seen = seen.into_inner().unwrap();
    // tiny_http chunks a body this large, so no length: the total is the catalogue's.
    assert_eq!(seen.first(), Some(&(0, model.bytes)));
    assert_eq!(seen.last(), Some(&(300_000, model.bytes)));
    // The VAD model is on disk now, so a second download skips it.
    std::fs::remove_file(&path).unwrap();
    download_from(
        &dir,
        model,
        &format!("{}/model.bin", server.origin()),
        "http://unused.invalid",
        |_, _| {},
    )
    .unwrap();
    assert_eq!(
        server.hits().iter().filter(|h| h.url == "/vad.bin").count(),
        1
    );
}

#[test]
fn a_failed_download_surfaces_and_leaves_nothing_behind() {
    let server = FakeServer::start(|hit| match hit.url.as_str() {
        "/vad.bin" => Reply::bytes(vec![1; 10]),
        _ => Reply::status(404, serde_json::json!({})),
    });
    let dir = Scratch::new("whisper-download-fail");
    let model = find("base").unwrap();
    let error = download_from(
        &dir,
        model,
        &format!("{}/missing.bin", server.origin()),
        &format!("{}/vad.bin", server.origin()),
        |_, _| {},
    )
    .unwrap_err();
    assert!(error.contains("404"), "{error}");
    assert!(!dir.join("ggml-base.bin").exists() && !dir.join("ggml-base.bin.part").exists());
    // The claim was released: the id is free to download again.
    assert!(!in_flight().contains(&"base".to_string()));
}

#[test]
fn a_second_download_of_one_model_is_refused_and_cancel_reaches_the_first() {
    let first = Claim::take("medium").unwrap();
    assert!(Claim::take("medium").is_err());
    assert!(cancel("medium"));
    assert!(first.1.load(Ordering::Relaxed));
    drop(first);
    assert!(!cancel("medium"));
}

#[test]
fn deleting_removes_the_model_and_its_partial() {
    let dir = Scratch::new("whisper-delete");
    std::fs::write(dir.join("ggml-small.bin"), vec![0; 10]).unwrap();
    std::fs::write(dir.join("ggml-small.bin.part"), vec![0; 5]).unwrap();
    std::fs::write(dir.join(VAD_FILE), b"v").unwrap();
    assert_eq!(delete(&dir, "small").unwrap(), 15);
    assert!(!dir.join("ggml-small.bin").exists());
    assert!(dir.join(VAD_FILE).exists());
    assert_eq!(delete(&dir, "small").unwrap(), 0);
    assert!(delete(&dir, "nope").is_err());
}

#[test]
fn this_machines_ram_is_read() {
    if cfg!(unix) {
        assert!(total_memory().unwrap() > GB);
    }
}

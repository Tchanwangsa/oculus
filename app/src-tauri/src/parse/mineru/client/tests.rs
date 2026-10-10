use super::errors::check_transfer_url;
use super::upload::UploadBody;
use super::*;
use crate::parse::{check_size, Phase, Progress, Skips};
use crate::providers::ratelimit::hold;
use crate::test_support::{write_pdf, FakeServer, Reply, Scratch};
use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Write};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn zip_of(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut archive = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, body) in entries {
            archive.start_file(*name, options).unwrap();
            archive.write_all(body).unwrap();
        }
        archive.finish().unwrap();
    }
    buffer.into_inner()
}

fn content_list(from: usize, pages: usize) -> Vec<u8> {
    let items: Vec<Value> = (0..pages)
        .map(|page| {
            json!({ "type": "text", "text": format!("page {}", from + page + 1),
                    "page_idx": page, "bbox": [100, 200, 900, 300] })
        })
        .collect();
    Value::Array(items).to_string().into_bytes()
}

fn client(fake: &FakeServer, ledger: Arc<UsageLedger>) -> MinerUCloud {
    MinerUCloud::new(&format!("{}/api/v4", fake.origin()), "test-only-token")
        .unwrap()
        .with_ledger(ledger)
        // Private buckets so one test cannot pace another.
        .with_buckets(
            Arc::new(TokenBucket::new(60_000.0)),
            Arc::new(TokenBucket::new(60_000.0)),
        )
        .with_time_scale(0.005)
}

fn ledger(scratch: &Scratch) -> Arc<UsageLedger> {
    Arc::new(UsageLedger::at(scratch.join("mineru-usage.json")))
}

#[test]
fn the_signed_put_carries_a_length_and_no_content_type() {
    let fake = FakeServer::start(|_| Reply::bytes(Vec::new()));
    let scratch = Scratch::new("cloud-put");
    let pdf = scratch.join("deck.pdf");
    fs::write(&pdf, vec![7u8; 3_000]).unwrap();

    let client = client(&fake, ledger(&scratch));
    let reported = Mutex::new(Vec::new());
    client
        .put_file(
            &format!("{}/upload/0?signature=private", fake.origin()),
            &pdf,
            &|sent| hold(&reported).push(sent),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        hold(&reported).last().copied(),
        Some(3_000),
        "the last byte always reports"
    );

    let hits = fake.hits();
    assert_eq!(hits.len(), 1);
    let hit = &hits[0];
    assert_eq!(hit.method, "PUT");
    assert_eq!(hit.header("content-type"), None, "{:?}", hit.headers);
    assert_eq!(hit.header("content-length"), Some("3000"));
    // A bearer token here would go to whatever storage host MinerU uses.
    assert_eq!(hit.header("authorization"), None);
    assert_eq!(hit.body.len(), 3_000);
}

#[test]
fn the_upload_body_reports_at_most_every_interval_and_always_the_end() {
    let reported = Mutex::new(Vec::new());
    let report = |sent| hold(&reported).push(sent);
    let mut body = UploadBody::new(
        std::io::Cursor::new(vec![1u8; 10_000]),
        10_000,
        Duration::from_secs(3600),
        &report,
        &|| false,
    );
    let mut chunk = [0u8; 1_000];
    while body.read(&mut chunk).unwrap() > 0 {}
    // The first read, then nothing until the last byte.
    assert_eq!(*hold(&reported), vec![1_000, 10_000]);
}

#[test]
fn a_skip_fails_the_upload_body_on_its_next_read() {
    let skipped = AtomicBool::new(false);
    let cancelled = || skipped.load(AtomicOrdering::SeqCst);
    let mut body = UploadBody::new(
        std::io::Cursor::new(vec![1u8; 4_000]),
        4_000,
        Duration::ZERO,
        &|_| {},
        &cancelled,
    );
    let mut chunk = [0u8; 1_000];
    assert_eq!(body.read(&mut chunk).unwrap(), 1_000);
    skipped.store(true, AtomicOrdering::SeqCst);
    let error = body.read(&mut chunk).unwrap_err();
    // `Interrupted` would be retried by `io::copy` instead of ending the PUT.
    assert_ne!(error.kind(), std::io::ErrorKind::Interrupted);
}

#[test]
fn a_skip_ends_the_wait_and_outlives_its_mark() {
    let scratch = Scratch::new("cloud-wait-skip");
    let pdf = scratch.join("waiting.pdf");
    let document = CloudDocument::new(&pdf, &scratch.join("images"), "images");
    let marker = {
        let pdf = pdf.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            Skips::shared().mark(&pdf);
        })
    };
    let started = Instant::now();
    let error = document.wait(&|_| {}).unwrap_err();
    marker.join().unwrap();
    assert!(matches!(error, ParseError::Cancelled), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    // A re-parse clears the mark; this abandoned document stays dropped.
    Skips::shared().clear(&pdf);
    assert!(document.cancelled());
}

#[test]
fn an_upload_url_that_is_not_signed_https_is_refused() {
    assert!(check_transfer_url("http://mineru.net/upload", "upload").is_err());
    // Hostname-less, in both shapes.
    assert!(check_transfer_url("https://", "upload").is_err());
    assert!(check_transfer_url("file:///etc/passwd", "upload").is_err());
    assert!(check_transfer_url("not a url", "upload").is_err());
    assert!(check_transfer_url("https://oss.example/upload?sig=x", "upload").is_ok());
}

#[test]
fn a_refused_token_is_never_retried() {
    for (code, expired) in [("A0211", true), ("A0202", false)] {
        let fake = FakeServer::start(move |_| {
            Reply::status(
                401,
                json!({ "msgCode": code, "msg": "user authenticate failed" }),
            )
        });
        let scratch = Scratch::new("cloud-auth");
        let client = client(&fake, ledger(&scratch));
        let error = client
            .api_json("GET", "/extract/task/x", None, &client.poll.clone())
            .unwrap_err();

        match error {
            ParseError::RejectedCredentials {
                code: seen,
                expired: seen_expired,
            } => {
                assert_eq!(seen.as_deref(), Some(code));
                assert_eq!(seen_expired, expired);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            fake.hits().len(),
            1,
            "a rejected token cannot be retried into working"
        );
    }
}

#[test]
fn the_quota_code_latches_the_ledger() {
    let fake = FakeServer::start(|_| Reply::json(json!({ "code": -60018, "msg": "no quota" })));
    let scratch = Scratch::new("cloud-quota");
    let book = ledger(&scratch);
    let client = client(&fake, book.clone());

    let error = client
        .api_json(
            "GET",
            "/extract-results/batch/x",
            None,
            &client.poll.clone(),
        )
        .unwrap_err();
    assert!(matches!(error, ParseError::QuotaExhausted));
    assert!(book.snapshot().quota_exhausted);
    // A ledger reopened over the same file still refuses, offline.
    let reopened = UsageLedger::at(book.path());
    assert!(matches!(
        reopened.ensure_available(1),
        Err(ParseError::QuotaExhausted)
    ));
}

#[test]
fn a_429_waits_and_does_not_spend_an_attempt() {
    let fake = FakeServer::start(|hit| {
        if hit.index < 6 {
            Reply::status(429, json!({ "msg": "slow down" })).with_header("Retry-After", "1")
        } else {
            Reply::json(json!({ "code": 0, "data": { "batch_id": "late" } }))
        }
    });
    let scratch = Scratch::new("cloud-429");
    let client = client(&fake, ledger(&scratch));

    let data = client
        .api_json(
            "GET",
            "/extract-results/batch/x",
            None,
            &client.poll.clone(),
        )
        .unwrap();
    assert_eq!(data["batch_id"], "late");
    // Six waits, past the four attempts a failure gets.
    assert_eq!(fake.hits().len(), 7);
}

#[test]
fn a_server_error_says_the_code_and_nothing_else() {
    let fake = FakeServer::start(|_| {
        Reply::json(json!({
            "code": -60099,
            "msg": "failed: https://oss.example/file?signature=secret-value",
        }))
    });
    let scratch = Scratch::new("cloud-codes");
    let client = client(&fake, ledger(&scratch));
    let error = client
        .api_json("GET", "/x", None, &client.poll.clone())
        .unwrap_err();

    let shown = error.to_string();
    assert!(shown.contains("-60099"), "{shown}");
    assert!(!shown.contains("signature"), "{shown}");
    assert!(!shown.contains("oss.example"), "{shown}");
}

#[test]
fn a_long_document_becomes_server_side_page_ranges() {
    let scratch = Scratch::new("cloud-split");
    let pdf = scratch.join("long.pdf");
    write_pdf(&pdf, 401);
    let fake = FakeServer::start(|_| Reply::bytes(Vec::new()));
    let client = client(&fake, ledger(&scratch));

    let document = CloudDocument::new(&pdf, &scratch.join("out"), "out");
    let tasks = client.build_tasks(0, &document).unwrap();

    assert_eq!(
        tasks
            .iter()
            .map(|task| (task.page_offset, task.page_count, task.page_ranges.clone()))
            .collect::<Vec<_>>(),
        vec![
            (0, 200, Some("1-200".into())),
            (200, 200, Some("201-400".into())),
            (400, 1, Some("401-401".into())),
        ]
    );
    assert_eq!(document.total_pages(), 401);
    // Every task uploads the whole file; the range is the server's job.
    assert!(tasks.iter().all(|task| task.source == pdf));
    assert!(tasks
        .iter()
        .all(|task| task.api_entry()["is_ocr"] == json!(false)));
    assert!(tasks.iter().all(|task| task.data_id.starts_with("oculus-")));

    // A document that fits in one task carries no range at all.
    let short = scratch.join("short.pdf");
    write_pdf(&short, 3);
    let document = CloudDocument::new(&short, &scratch.join("out"), "out");
    let tasks = client.build_tasks(0, &document).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].page_ranges, None);
    assert_eq!(tasks[0].api_entry().get("page_ranges"), None);
}

#[test]
fn an_oversized_document_is_refused_before_anything_is_sent() {
    let scratch = Scratch::new("cloud-big");
    let pdf = scratch.join("huge.pdf");
    write_pdf(&pdf, 1);
    let fake = FakeServer::start(|_| Reply::bytes(Vec::new()));
    let client = client(&fake, ledger(&scratch));
    let document = CloudDocument::new(&pdf, &scratch.join("out"), "out");

    // A 1-byte ceiling stands in for the real one.
    assert!(matches!(
        check_size(&pdf, 1),
        Err(ParseError::TooLarge { limit_bytes: 1, .. })
    ));
    // Under the real limit the same file builds a task.
    assert_eq!(client.build_tasks(0, &document).unwrap().len(), 1);
}

#[test]
fn page_count_refuses_junk_and_pageless_files() {
    let scratch = Scratch::new("cloud-count");
    let pdf = scratch.join("three.pdf");
    write_pdf(&pdf, 3);
    assert_eq!(archive::page_count(&pdf).unwrap(), 3);

    let junk = scratch.join("junk.pdf");
    fs::write(&junk, b"not a pdf at all").unwrap();
    assert!(
        matches!(archive::page_count(&junk), Err(ParseError::Document { ref code }) if code == "unreadable-pdf")
    );

    let empty = scratch.join("empty.pdf");
    write_pdf(&empty, 0);
    assert!(
        matches!(archive::page_count(&empty), Err(ParseError::Document { ref code }) if code == "empty-pdf")
    );
}

/// Submit → upload → poll → download → collect, with the two tasks of one
/// document completing in the wrong order.
#[test]
fn a_batch_runs_end_to_end_and_sums_progress_monotonically() {
    let scratch = Scratch::new("cloud-e2e");
    let pdf = scratch.join("deck.pdf");
    write_pdf(&pdf, 3);

    let zips = vec![
        zip_of(&[("result/deck_content_list.json", content_list(0, 2))]),
        zip_of(&[("nested/deck_content_list.json", content_list(2, 1))]),
    ];
    let submitted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let polls = Arc::new(AtomicU64::new(0));

    let fake = {
        let submitted = submitted.clone();
        let polls = polls.clone();
        FakeServer::start(move |hit| {
            let origin = hit.origin.clone();
            if hit.method == "POST" {
                assert_eq!(hit.url, "/api/v4/file-urls/batch");
                let body = hit.json();
                assert_eq!(body["language"], "ch");
                assert_eq!(body["model_version"], "pipeline");
                assert_eq!(body["enable_formula"], json!(true));
                assert_eq!(body["enable_table"], json!(true));
                let files = body["files"].as_array().unwrap().clone();
                *hold(&submitted) = files
                    .iter()
                    .map(|file| file["data_id"].as_str().unwrap().to_string())
                    .collect();
                return Reply::json(json!({
                    "code": 0,
                    "data": {
                        "batch_id": "batch-1",
                        "file_urls": [
                            format!("{origin}/upload/0"),
                            format!("{origin}/upload/1"),
                        ],
                    },
                }));
            }
            if hit.method == "PUT" {
                return Reply::bytes(Vec::new());
            }
            if hit.url.starts_with("/result/") {
                let index: usize = hit
                    .url
                    .trim_start_matches("/result/")
                    .trim_end_matches(".zip")
                    .parse()
                    .unwrap();
                return Reply::bytes(zips[index].clone());
            }
            assert_eq!(hit.url, "/api/v4/extract-results/batch/batch-1");
            let ids = hold(&submitted).clone();
            let round = polls.fetch_add(1, AtomicOrdering::SeqCst);
            if round == 0 {
                // The second task finishes first, and the first is still
                // running: the sum must not go backwards when it lands.
                return Reply::json(json!({ "code": 0, "data": { "extract_result": [
                    { "data_id": ids[1], "state": "done", "full_zip_url": format!("{origin}/result/1.zip") },
                    { "data_id": ids[0], "state": "running", "extract_progress": { "extracted_pages": 1 } },
                ]}}));
            }
            Reply::json(json!({ "code": 0, "data": { "extract_result": [
                { "data_id": ids[0], "state": "done", "full_zip_url": format!("{origin}/result/0.zip") },
            ]}}))
        })
    };

    let book = ledger(&scratch);
    let client = client(&fake, book.clone()).with_pages_per_task(2);
    let document = CloudDocument::new(&pdf, &scratch.join("deck_images"), "deck_images");

    // The batcher's split, by hand: a worker parses, this thread `wait`s.
    let worker = {
        let client = client.clone();
        let document = document.clone();
        std::thread::spawn(move || {
            let mut results = client.extract_documents(&[document.clone()]);
            document.finish(results.remove(0));
        })
    };
    let seen: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
    let phases: Mutex<Vec<Progress>> = Mutex::new(Vec::new());
    let output = document
        .wait(&|progress| {
            hold(&seen).push((progress.pages_done, progress.total_pages));
            hold(&phases).push(progress);
        })
        .unwrap();
    worker.join().unwrap();

    // Phases only move forward, and the upload ends at 100% — the whole
    // file once per task — before processing, however events coalesced.
    let phases = hold(&phases).clone();
    let order = |phase| match phase {
        Phase::UploadWait => 0,
        Phase::Uploading => 1,
        Phase::Processing => 2,
    };
    assert!(phases
        .windows(2)
        .all(|pair| order(pair[0].phase) <= order(pair[1].phase)));
    let size = fs::metadata(&pdf).unwrap().len();
    assert!(
        phases.iter().any(|p| p.phase == Phase::Uploading
            && p.bytes_done == 2 * size
            && p.bytes_total == 2 * size),
        "{phases:?}"
    );
    assert_eq!(phases.last().map(|p| p.phase), Some(Phase::Processing));

    assert_eq!(output.total_pages, 3);
    assert_eq!(
        output
            .pages
            .iter()
            .map(|page| page.markdown.as_str())
            .collect::<Vec<_>>(),
        vec!["page 1", "page 2", "page 3"]
    );
    // The bbox survives `page_idx` rebasing, the second task's page included.
    assert!(output
        .pages
        .iter()
        .all(|page| page.blocks.len() == 1 && page.blocks[0].bbox == [0.1, 0.2, 0.9, 0.3]));
    assert_eq!(output.image_count, 0);

    let seen = hold(&seen).clone();
    assert!(!seen.is_empty(), "progress was never reported");
    assert!(
        seen.windows(2).all(|pair| pair[0].0 <= pair[1].0),
        "progress went backwards: {seen:?}"
    );
    assert_eq!(seen.last().copied(), Some((3, 3)), "{seen:?}");
    assert!(seen.iter().all(|(done, _)| *done <= 3));

    // Two tasks, three pages, reserved before the first POST.
    let usage = book.snapshot();
    assert_eq!(usage.files, 2);
    assert_eq!(usage.pages, 3);
}

/// One bad document must not fail the rest of its batch.
#[test]
fn one_bad_document_does_not_take_the_batch_with_it() {
    let scratch = Scratch::new("cloud-isolation");
    let mut pdfs = Vec::new();
    for name in ["a.pdf", "b.pdf", "c.pdf"] {
        let path = scratch.join(name);
        write_pdf(&path, 1);
        pdfs.push(path);
    }

    // `a` comes back with a page index outside its own task; `b` fails
    // outright; `c` is fine.
    let broken = Value::Array(vec![
        json!({ "type": "text", "text": "stray", "page_idx": 9 }),
    ])
    .to_string()
    .into_bytes();
    let zips = vec![
        zip_of(&[("r/a_content_list.json", broken)]),
        zip_of(&[("r/c_content_list.json", content_list(0, 1))]),
    ];
    let submitted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let fake = {
        let submitted = submitted.clone();
        FakeServer::start(move |hit| {
            let origin = hit.origin.clone();
            if hit.method == "POST" {
                let files = hit.json()["files"].as_array().unwrap().clone();
                *hold(&submitted) = files
                    .iter()
                    .map(|file| file["data_id"].as_str().unwrap().to_string())
                    .collect();
                return Reply::json(json!({ "code": 0, "data": {
                    "batch_id": "b",
                    "file_urls": (0..files.len()).map(|i| format!("{origin}/upload/{i}")).collect::<Vec<_>>(),
                }}));
            }
            if hit.method == "PUT" {
                return Reply::bytes(Vec::new());
            }
            if hit.url.starts_with("/result/") {
                let index: usize = hit
                    .url
                    .trim_start_matches("/result/")
                    .trim_end_matches(".zip")
                    .parse()
                    .unwrap();
                return Reply::bytes(zips[index].clone());
            }
            let ids = hold(&submitted).clone();
            Reply::json(json!({ "code": 0, "data": { "extract_result": [
                { "data_id": ids[0], "state": "done", "full_zip_url": format!("{origin}/result/0.zip") },
                { "data_id": ids[1], "state": "failed", "err_msg": "unsupported" },
                { "data_id": ids[2], "state": "done", "full_zip_url": format!("{origin}/result/1.zip") },
            ]}}))
        })
    };

    let client = client(&fake, ledger(&scratch));
    let documents: Vec<Arc<CloudDocument>> = pdfs
        .iter()
        .map(|pdf| CloudDocument::new(pdf, &scratch.join("images"), "images"))
        .collect();
    let results = client.extract_documents(&documents);

    assert!(
        matches!(&results[0], Err(ParseError::Document { code }) if code == "page-index-out-of-range"),
        "{:?}",
        results[0].as_ref().err().map(|e| e.to_string())
    );
    assert!(
        matches!(&results[1], Err(ParseError::Document { code }) if code == "task-failed"),
        "{:?}",
        results[1].as_ref().err().map(|e| e.to_string())
    );
    let good = results[2]
        .as_ref()
        .expect("the third document was unaffected");
    assert_eq!(good.pages.len(), 1);
    assert_eq!(good.pages[0].markdown, "page 1");
}

/// Serves one batch: records the POSTed `data_id`s and the PUT order,
/// and answers every poll with `done` and the same one-page result.
fn recording_server(
    submitted: Arc<Mutex<Vec<String>>>,
    puts: Arc<Mutex<Vec<String>>>,
) -> FakeServer {
    let zip = zip_of(&[("r/x_content_list.json", content_list(0, 1))]);
    FakeServer::start(move |hit| {
        let origin = hit.origin.clone();
        if hit.method == "POST" {
            let files = hit.json()["files"].as_array().unwrap().clone();
            *hold(&submitted) = files
                .iter()
                .map(|file| file["data_id"].as_str().unwrap().to_string())
                .collect();
            return Reply::json(json!({ "code": 0, "data": {
                "batch_id": "b",
                "file_urls": (0..files.len()).map(|i| format!("{origin}/upload/{i}")).collect::<Vec<_>>(),
            }}));
        }
        if hit.method == "PUT" {
            hold(&puts).push(hit.url.clone());
            return Reply::bytes(Vec::new());
        }
        if hit.url.starts_with("/result/") {
            return Reply::bytes(zip.clone());
        }
        let results: Vec<Value> = hold(&submitted)
            .iter()
            .map(|id| json!({ "data_id": id, "state": "done", "full_zip_url": format!("{origin}/result/0.zip") }))
            .collect();
        Reply::json(json!({ "code": 0, "data": { "extract_result": results }}))
    })
}

#[test]
fn the_smallest_file_is_uploaded_first() {
    let scratch = Scratch::new("cloud-order");
    let large = scratch.join("large.pdf");
    write_pdf(&large, 40);
    let small = scratch.join("small.pdf");
    write_pdf(&small, 1);
    assert!(fs::metadata(&large).unwrap().len() > fs::metadata(&small).unwrap().len());

    let puts = Arc::new(Mutex::new(Vec::new()));
    let fake = recording_server(Arc::new(Mutex::new(Vec::new())), puts.clone());
    let client = client(&fake, ledger(&scratch));
    let documents: Vec<Arc<CloudDocument>> = [&large, &small]
        .iter()
        .map(|pdf| CloudDocument::new(pdf, &scratch.join("images"), "images"))
        .collect();
    let results = client.extract_documents(&documents);
    assert!(results.iter().all(Result::is_ok));
    // The URLs come back in submit order: `large` is 0, `small` is 1.
    assert_eq!(*hold(&puts), vec!["/upload/1", "/upload/0"]);
}

#[test]
fn a_skipped_document_is_never_reserved_submitted_or_uploaded() {
    let scratch = Scratch::new("cloud-skip");
    let skipped = scratch.join("skipped.pdf");
    write_pdf(&skipped, 4);
    let kept = scratch.join("kept.pdf");
    write_pdf(&kept, 1);
    Skips::shared().mark(&skipped);

    let submitted = Arc::new(Mutex::new(Vec::new()));
    let puts = Arc::new(Mutex::new(Vec::new()));
    let fake = recording_server(submitted.clone(), puts.clone());
    let book = ledger(&scratch);
    let client = client(&fake, book.clone());
    let documents: Vec<Arc<CloudDocument>> = [&skipped, &kept]
        .iter()
        .map(|pdf| CloudDocument::new(pdf, &scratch.join("images"), "images"))
        .collect();
    let results = client.extract_documents(&documents);
    Skips::shared().clear(&skipped);

    assert!(matches!(results[0], Err(ParseError::Cancelled)));
    assert!(results[1].is_ok());
    assert_eq!(hold(&submitted).len(), 1);
    assert_eq!(hold(&puts).len(), 1);
    let usage = book.snapshot();
    assert_eq!(
        (usage.files, usage.pages),
        (1, 1),
        "only the kept file was reserved"
    );
}

#[test]
fn a_rejected_token_fails_the_whole_batch() {
    let scratch = Scratch::new("cloud-batch-auth");
    let mut documents = Vec::new();
    for name in ["a.pdf", "b.pdf"] {
        let path = scratch.join(name);
        write_pdf(&path, 1);
        documents.push(CloudDocument::new(&path, &scratch.join("images"), "images"));
    }
    let fake = FakeServer::start(|_| Reply::status(403, json!({ "msgCode": "A0202" })));
    let client = client(&fake, ledger(&scratch));

    let results = client.extract_documents(&documents);
    assert_eq!(results.len(), 2);
    for result in &results {
        let error = result
            .as_ref()
            .err()
            .expect("credentials condemn every file");
        assert!(matches!(
            error,
            ParseError::RejectedCredentials { expired: false, .. }
        ));
        assert!(error.latching());
    }
}

#[test]
fn a_failed_submit_still_burns_its_reservation() {
    let scratch = Scratch::new("cloud-burn");
    let pdf = scratch.join("a.pdf");
    write_pdf(&pdf, 5);
    let fake = FakeServer::start(|_| Reply::status(500, json!({ "msg": "server" })));
    let book = ledger(&scratch);
    let client = client(&fake, book.clone());
    let document = CloudDocument::new(&pdf, &scratch.join("images"), "images");

    let results = client.extract_documents(&[document]);
    assert!(results[0].is_err());
    // The POST may have been counted server-side; uncertain failures count.
    let usage = book.snapshot();
    assert_eq!(usage.files, 1);
    assert_eq!(usage.pages, 5);
}

#[test]
fn a_zip_that_climbs_out_of_its_directory_is_refused() {
    let scratch = Scratch::new("cloud-slip");
    let archive = scratch.join("bad.zip");
    fs::write(&archive, zip_of(&[("../escape.txt", b"bad".to_vec())])).unwrap();

    let error = safe_extract(&archive, &scratch.join("out")).unwrap_err();
    assert!(matches!(error, ParseError::Document { .. }), "{error}");
    assert!(!scratch.join("escape.txt").exists());
}

#[test]
fn the_content_list_is_found_by_shape_not_by_name() {
    let scratch = Scratch::new("cloud-glob");
    let root = scratch.join("result");
    fs::create_dir_all(root.join("deep/nested")).unwrap();
    // Never the flat markdown beside it.
    fs::write(root.join("deck.md"), "# not this").unwrap();
    fs::write(root.join("deep/nested/whatever_content_list.json"), "[]").unwrap();

    assert_eq!(
        find_content_list(&root),
        Some(root.join("deep/nested/whatever_content_list.json"))
    );
    assert_eq!(find_content_list(&scratch.join("missing")), None);
}

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use super::*;
use crate::test_support::Scratch;

/// A `block_on` on a runtime worker panics the task; surviving the call is
/// what is under test, so it passes with no database.
#[test]
fn the_config_is_readable_from_inside_an_async_runtime() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let base = runtime.block_on(async { parse_config().base_url });
    assert!(
        !base.is_empty(),
        "a backend always resolves to some API root"
    );
}

/// A real scratch folder: the staging rename must be same-filesystem.
fn scratch(name: &str) -> Scratch {
    Scratch::new(&format!("parse-{name}"))
}

fn sample_pdf(dir: &Path) -> PathBuf {
    let pdf = dir.join("Lecture 3.pdf");
    fs::write(&pdf, b"%PDF-1.4").unwrap();
    pdf
}

#[test]
fn artifact_names_match_the_records_already_on_disk() {
    let pdf = Path::new("/library/subj/Lecture 3.pdf");
    assert_eq!(md_path(pdf), Path::new("/library/subj/Lecture 3.md"));
    assert_eq!(
        pages_path(pdf),
        Path::new("/library/subj/Lecture 3.pages.json")
    );
    assert_eq!(
        images_dir_for(pdf),
        Path::new("/library/subj/Lecture 3_images")
    );
}

#[test]
fn pages_are_ordered_and_gap_filled() {
    let pdf = Path::new("/library/Lecture 3.pdf");
    let out = ParseOutput::new(
        pdf,
        4,
        vec![
            ParsePage {
                page_no: 3,
                markdown: "three".into(),
            },
            ParsePage {
                page_no: 1,
                markdown: "one".into(),
            },
            // Out of range: a backend that split the document and got its
            // offsets wrong must not be able to corrupt the join key.
            ParsePage {
                page_no: 9,
                markdown: "nine".into(),
            },
        ],
        Some("mineru-cloud".into()),
        2,
    );
    assert_eq!(out.page_count, 4);
    assert_eq!(
        out.pages.iter().map(|p| p.page_no).collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(out.pages[1].markdown, "");
    // Blank pages keep their slot, so the `.md` and the records stay in step.
    assert_eq!(out.document_markdown(), "one\n\n\n\nthree\n\n");
}

#[test]
fn record_keeps_the_python_wire_shape() {
    let pdf = Path::new("/library/Lecture 3.pdf");
    let out = ParseOutput::new(
        pdf,
        1,
        vec![ParsePage {
            page_no: 1,
            markdown: "ψ".into(),
        }],
        None,
        0,
    );
    let json = serde_json::to_string(&out).unwrap();
    // Non-ASCII unescaped, as `ensure_ascii=False` wrote it.
    assert!(json.contains("\"ψ\""), "{json}");
    assert!(!json.contains("backend"), "{json}");
    assert!(!json.contains("image_count"), "{json}");
    assert!(json.contains("\"parser_version\":2"), "{json}");
}

#[test]
fn write_swaps_images_and_lands_the_record_last() {
    let dir = scratch("write");
    let pdf = sample_pdf(&dir);

    // A previous parse's artifacts, which this one replaces.
    fs::create_dir_all(images_dir_for(&pdf)).unwrap();
    fs::write(images_dir_for(&pdf).join("old.jpg"), b"old").unwrap();

    let staging = ImageStaging::begin(&pdf).unwrap();
    assert_eq!(staging.rel(), "Lecture 3_images");
    fs::write(staging.dir().join("new.jpg"), b"new").unwrap();

    let out = ParseOutput::new(
        &pdf,
        1,
        vec![ParsePage {
            page_no: 1,
            markdown: "![](Lecture 3_images/new.jpg)".into(),
        }],
        Some("mineru-cloud".into()),
        1,
    );
    out.write(&pdf, staging).unwrap();

    assert!(images_dir_for(&pdf).join("new.jpg").is_file());
    assert!(!images_dir_for(&pdf).join("old.jpg").exists());
    assert!(md_path(&pdf).is_file());
    assert_eq!(read_record(&pdf).unwrap().parser_version, PARSER_VERSION);
    assert_eq!(parse_mode(&pdf), Some(MODE));
    // No temp record left behind.
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".tmp") || n.starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn a_failed_parse_leaves_the_previous_artifacts_alone() {
    let dir = scratch("abandon");
    let pdf = sample_pdf(&dir);
    fs::create_dir_all(images_dir_for(&pdf)).unwrap();
    fs::write(images_dir_for(&pdf).join("old.jpg"), b"old").unwrap();
    fs::write(md_path(&pdf), "previous").unwrap();

    {
        let staging = ImageStaging::begin(&pdf).unwrap();
        fs::write(staging.dir().join("half.jpg"), b"half").unwrap();
        // Dropped without committing, as an `Err` out of `parse` would.
    }

    assert!(images_dir_for(&pdf).join("old.jpg").is_file());
    assert_eq!(fs::read_to_string(md_path(&pdf)).unwrap(), "previous");
    let entries: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with('.'))
        .collect();
    assert!(entries.is_empty(), "staging survived: {entries:?}");
}

#[test]
fn version_mismatch_is_refused_and_names_both_sides() {
    let health = Health {
        backend: "oculus-local".into(),
        parser_version: PARSER_VERSION + 1,
        ready: true,
    };
    let error = health.check().unwrap_err();
    assert!(matches!(error, ParseError::VersionMismatch { .. }));
    let shown = error.to_string();
    assert!(shown.contains(&(PARSER_VERSION + 1).to_string()), "{shown}");
    assert!(shown.contains(&PARSER_VERSION.to_string()), "{shown}");
    assert!(!error.retryable());
    assert!(error.latching());
}

#[test]
fn only_a_quality_record_counts_as_parsed() {
    let dir = scratch("mode");
    let pdf = sample_pdf(&dir);

    // No record at all.
    assert_eq!(parse_mode(&pdf), None);

    // Markdown and an images directory are not evidence: both survive a
    // crash that never wrote a record.
    fs::write(md_path(&pdf), "half a parse").unwrap();
    fs::create_dir_all(images_dir_for(&pdf)).unwrap();
    assert_eq!(parse_mode(&pdf), None);

    // The retired tier, and unreadable JSON, both mean "parse it".
    fs::write(pages_path(&pdf), r#"{"mode":"fast","parser_version":2}"#).unwrap();
    assert_eq!(parse_mode(&pdf), None);
    fs::write(pages_path(&pdf), "{ not json").unwrap();
    assert_eq!(parse_mode(&pdf), None);

    // Done regardless of the version that wrote it.
    fs::write(pages_path(&pdf), r#"{"mode":"quality","parser_version":1}"#).unwrap();
    assert_eq!(parse_mode(&pdf), Some("quality"));
}

#[test]
fn a_second_parse_of_the_same_pdf_waits_for_the_first() {
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;
    use std::time::Duration;

    let in_flight: &'static InFlight = Box::leak(Box::new(InFlight::new()));
    let live = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let threads: Vec<_> = (0..3)
        .map(|_| {
            let (live, peak) = (live.clone(), peak.clone());
            std::thread::spawn(move || {
                let _claim = in_flight.claim(Path::new("/library/a.pdf"));
                let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(50));
                live.fetch_sub(1, Ordering::SeqCst);
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(peak.load(Ordering::SeqCst), 1);

    // Another PDF is not held up, and a panicking holder still frees its PDF.
    let _a = in_flight.claim(Path::new("/library/a.pdf"));
    let _b = in_flight.claim(Path::new("/library/b.pdf"));
    let crashed = std::thread::spawn(move || {
        let _claim = in_flight.claim(Path::new("/library/c.pdf"));
        panic!("parser crashed");
    });
    assert!(crashed.join().is_err());
    let _c = in_flight.claim(Path::new("/library/c.pdf"));
}

#[test]
fn a_lost_markdown_is_rebuilt_from_the_record() {
    let dir = scratch("restore-md");
    let pdf = sample_pdf(&dir);
    let out = ParseOutput::new(
        &pdf,
        2,
        vec![
            ParsePage {
                page_no: 1,
                markdown: "one".into(),
            },
            ParsePage {
                page_no: 2,
                markdown: "two".into(),
            },
        ],
        None,
        0,
    );
    out.write(&pdf, ImageStaging::begin(&pdf).unwrap()).unwrap();
    fs::remove_file(md_path(&pdf)).unwrap();

    let record = read_record(&pdf).unwrap();
    assert!(record.restore_markdown(&pdf).unwrap());
    assert_eq!(fs::read_to_string(md_path(&pdf)).unwrap(), "one\n\ntwo");
    // Present already: left alone.
    fs::write(md_path(&pdf), "edited").unwrap();
    assert!(!record.restore_markdown(&pdf).unwrap());
    assert_eq!(fs::read_to_string(md_path(&pdf)).unwrap(), "edited");
}

#[test]
fn a_failed_conversion_is_a_document_failure_that_does_not_blame_mineru() {
    let error = ParseError::Document {
        code: CONVERSION_FAILED.into(),
    };
    assert_eq!(error.kind(), "document");
    assert!(!error.retryable());
    assert!(!error.latching());
    assert!(!error.to_string().contains("MinerU"), "{error}");
}

#[test]
fn an_unreadable_spreadsheet_is_a_document_failure_that_does_not_blame_mineru() {
    let error = ParseError::Document {
        code: SHEET_UNREADABLE.into(),
    };
    assert_eq!(error.kind(), "document");
    assert!(!error.retryable());
    assert!(!error.to_string().contains("MinerU"), "{error}");
}

#[test]
fn a_skip_is_neither_retried_nor_latching() {
    let error = ParseError::Cancelled;
    assert_eq!(error.kind(), "cancelled");
    assert!(!error.retryable());
    assert!(!error.latching());
    assert!(error.to_string().starts_with("Skipped"), "{error}");
}

#[test]
fn a_skip_mark_holds_until_cleared() {
    let skips = Skips::new();
    let pdf = Path::new("/library/skip-me.pdf");
    assert!(!skips.is_marked(pdf));
    skips.mark(pdf);
    skips.mark(pdf);
    assert!(skips.is_marked(pdf));
    assert!(!skips.is_marked(Path::new("/library/other.pdf")));
    skips.clear(pdf);
    assert!(!skips.is_marked(pdf));
}

#[test]
fn a_stale_fallback_policy_is_not_an_engine() {
    assert_eq!(Engine::parse("auto"), None);
    assert_eq!(Engine::parse("cloud"), Some(Engine::Cloud));
    assert_eq!(Engine::parse("local"), Some(Engine::Local));
}

#[test]
fn oversized_files_are_refused_with_both_numbers() {
    let dir = scratch("size");
    let pdf = dir.join("big.pdf");
    fs::write(&pdf, vec![0u8; 2048]).unwrap();
    let error = check_size(&pdf, 1024).unwrap_err();
    assert!(matches!(
        error,
        ParseError::TooLarge {
            bytes: 2048,
            limit_bytes: 1024
        }
    ));
    assert!(!error.retryable());
    assert!(error.to_string().contains("MB"), "{error}");
    assert_eq!(check_size(&pdf, 4096).unwrap(), 2048);
}

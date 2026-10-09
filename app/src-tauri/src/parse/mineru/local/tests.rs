use super::request::*;
use super::*;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::test_support::{dead_origin, write_pdf, FakeServer, Scratch};

use serde_json::json;

/// The archive MinerU answers with: `{stem}/{parse_dir}/…`.
fn result_zip(stem: &str, content: Value) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut archive = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        archive
            .start_file(format!("{stem}/auto/{stem}_content_list.json"), options)
            .unwrap();
        archive.write_all(content.to_string().as_bytes()).unwrap();
        archive.finish().unwrap();
    }
    buffer.into_inner()
}

#[test]
fn the_envelope_brackets_the_file_with_the_boundary() {
    let (head, tail) = envelope("BOUND", "Lecture \"3\".pdf");
    let head = String::from_utf8(head).unwrap();
    assert!(head.starts_with("--BOUND\r\n"), "{head}");
    assert!(head.contains("filename=\"Lecture 3.pdf\""), "{head}");
    assert!(head.ends_with("\r\n\r\n"), "{head}");
    assert_eq!(String::from_utf8(tail).unwrap(), "\r\n--BOUND--\r\n");
}

#[test]
fn the_post_carries_every_flag_and_the_file_itself() {
    let dir = Scratch::new("local-request");
    let pdf = dir.join("Lecture 3.pdf");
    write_pdf(&pdf, 2);
    let bytes = fs::read(&pdf).unwrap();

    let content = json!([{ "type": "text", "text": "Hello", "page_idx": 0 }]);
    let zip = result_zip("Lecture 3", content);
    let server = FakeServer::start(move |_| (200, zip.clone()));

    let client = MinerULocal::new(&server.origin());
    let archive = dir.join("out.zip");
    client.post_file_parse(&pdf, &archive).unwrap();

    let hits = server.hits();
    let hit = hits.first().expect("nothing was posted");
    assert_eq!(hit.method, "POST");
    assert_eq!(hit.url, "/file_parse");

    let content_type = hit.header("content-type").unwrap_or_default();
    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .expect(content_type)
        .to_string();
    let body = String::from_utf8_lossy(&hit.body);
    assert!(
        body.starts_with(&format!("--{boundary}\r\n")),
        "{}",
        &body[..80]
    );
    assert!(body.ends_with(&format!("\r\n--{boundary}--\r\n")));

    for (name, value) in FIELDS {
        assert!(
            body.contains(&format!("name=\"{name}\"\r\n\r\n{value}\r\n")),
            "{name} is not in the body"
        );
    }
    assert!(
        body.contains("name=\"files\"; filename=\"Lecture 3.pdf\""),
        "no file part"
    );
    assert!(
        hit.body.windows(bytes.len()).any(|window| window == bytes),
        "the PDF is not in the body"
    );
}

#[test]
fn the_zip_becomes_page_records_through_the_shared_renderer() {
    let dir = Scratch::new("local-render");
    let pdf = dir.join("Lecture 3.pdf");
    write_pdf(&pdf, 3);

    // A trailing blank page: nothing on page 3 to be counted by.
    let content = json!([
        { "type": "text", "text": "First slide", "page_idx": 0, "bbox": [100, 200, 900, 300] },
        { "type": "text", "text": "Second slide", "page_idx": 1 },
    ]);
    let zip = result_zip("Lecture 3", content);
    let server = FakeServer::start(move |_| (200, zip.clone()));

    let images = dir.join("Lecture 3_images");
    let seen: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
    let output = MinerULocal::new(&server.origin())
        .parse(&pdf, &images, "Lecture 3_images", &|progress| {
            seen.lock()
                .unwrap()
                .push((progress.pages_done, progress.total_pages));
        })
        .unwrap();

    assert_eq!(output.page_count, 3);
    assert_eq!(output.pages.len(), 3);
    assert_eq!(
        output
            .pages
            .iter()
            .map(|page| page.page_no)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(
        output.pages[0].markdown.contains("First slide"),
        "{:?}",
        output.pages[0]
    );
    assert!(
        output.pages[1].markdown.contains("Second slide"),
        "{:?}",
        output.pages[1]
    );
    assert_eq!(output.pages[2].markdown, "");
    // Blocks reach the record; a boxless item and a blank page have none.
    assert_eq!(output.pages[0].blocks.len(), 1);
    assert_eq!(output.pages[0].blocks[0].bbox, [0.1, 0.2, 0.9, 0.3]);
    assert!(output.pages[1].blocks.is_empty() && output.pages[2].blocks.is_empty());
    assert_eq!(output.backend.as_deref(), Some(BACKEND));
    assert_eq!(output.parser_version, PARSER_VERSION);

    assert_eq!(*seen.lock().unwrap(), vec![(0, 3), (3, 3)]);
}

#[test]
fn a_result_that_is_not_an_archive_condemns_only_this_document() {
    let dir = Scratch::new("local-garbage");
    let pdf = dir.join("Lecture 3.pdf");
    write_pdf(&pdf, 1);
    let server = FakeServer::start(|_| (200, b"not a zip at all".to_vec()));

    let error = MinerULocal::new(&server.origin())
        .parse(&pdf, &dir.join("images"), "images", &|_| {})
        .unwrap_err();
    assert_eq!(error.kind(), "document");
    assert!(!error.latching());
}

#[test]
fn an_archive_with_no_content_list_is_a_document_failure() {
    let dir = Scratch::new("local-empty-zip");
    let pdf = dir.join("Lecture 3.pdf");
    write_pdf(&pdf, 1);

    let mut buffer = Cursor::new(Vec::new());
    zip::ZipWriter::new(&mut buffer).finish().unwrap();
    let zip = buffer.into_inner();
    let server = FakeServer::start(move |_| (200, zip.clone()));

    let error = MinerULocal::new(&server.origin())
        .parse(&pdf, &dir.join("images"), "images", &|_| {})
        .unwrap_err();
    assert!(matches!(&error, ParseError::Document { code } if code == "no-content-list"));
}

#[test]
fn a_skipped_pdf_is_never_posted() {
    let dir = Scratch::new("local-skip-before");
    let pdf = dir.join("Skipped.pdf");
    write_pdf(&pdf, 1);
    let server = FakeServer::start(|_| (500, Vec::new()));
    crate::parse::Skips::shared().mark(&pdf);
    let error = MinerULocal::new(&server.origin())
        .parse(&pdf, &dir.join("images"), "images", &|_| {})
        .unwrap_err();
    crate::parse::Skips::shared().clear(&pdf);
    assert!(matches!(error, ParseError::Cancelled), "{error}");
    assert!(server.hits().is_empty());
}

#[test]
fn a_result_that_lands_after_a_skip_is_discarded() {
    let dir = Scratch::new("local-skip-during");
    let pdf = dir.join("Late.pdf");
    write_pdf(&pdf, 1);
    let zip = result_zip(
        "Late",
        json!([{ "type": "text", "text": "x", "page_idx": 0 }]),
    );
    let server = {
        let pdf = pdf.clone();
        // The skip arrives while the server is still parsing.
        FakeServer::start(move |_| {
            crate::parse::Skips::shared().mark(&pdf);
            (200, zip.clone())
        })
    };
    let seen: Mutex<Vec<crate::parse::Phase>> = Mutex::new(Vec::new());
    let error = MinerULocal::new(&server.origin())
        .parse(&pdf, &dir.join("images"), "images", &|progress| {
            seen.lock().unwrap().push(progress.phase);
        })
        .unwrap_err();
    crate::parse::Skips::shared().clear(&pdf);
    assert!(matches!(error, ParseError::Cancelled), "{error}");
    assert!(!dir.join("images").exists(), "nothing was rendered");
    assert_eq!(*seen.lock().unwrap(), vec![crate::parse::Phase::Processing]);
}

#[test]
fn a_refused_connection_is_offline_not_a_broken_document() {
    let dir = Scratch::new("local-offline");
    let pdf = dir.join("Lecture 3.pdf");
    write_pdf(&pdf, 1);

    let error = MinerULocal::new(&dead_origin())
        .parse(&pdf, &dir.join("images"), "images", &|_| {})
        .unwrap_err();
    assert_eq!(error.kind(), "offline");
    assert!(error.retryable());
    assert!(!error.latching());
}

#[test]
fn a_4xx_is_this_document_and_a_5xx_is_the_server() {
    let dir = Scratch::new("local-http");
    let pdf = dir.join("Lecture 3.pdf");
    write_pdf(&pdf, 1);

    let refused = FakeServer::start(|_| (409, b"{\"detail\":\"http://signed.example\"}".to_vec()));
    let error = MinerULocal::new(&refused.origin())
        .parse(&pdf, &dir.join("images"), "images", &|_| {})
        .unwrap_err();
    assert!(matches!(&error, ParseError::Document { code } if code == "local-http-409"));
    assert!(!error.to_string().contains("signed.example"), "{error}");

    let broken = FakeServer::start(|_| (500, Vec::new()));
    let error = MinerULocal::new(&broken.origin())
        .parse(&pdf, &dir.join("images"), "images", &|_| {})
        .unwrap_err();
    assert_eq!(error.kind(), "not_ready");
    assert!(
        error.retryable(),
        "a 500 is the server's, and the file deserves another go"
    );
}

#[test]
fn a_redirect_is_not_a_result_and_not_a_broken_document() {
    // See the 3xx arm in `post_file_parse`.
    let dir = Scratch::new("local-redirect");
    let pdf = dir.join("Lecture 4.pdf");
    write_pdf(&pdf, 1);

    let moved = FakeServer::start(|_| (302, b"<html>moved</html>".to_vec()));
    let error = MinerULocal::new(&moved.origin())
        .parse(&pdf, &dir.join("images"), "images", &|_| {})
        .unwrap_err();
    assert_eq!(
        error.kind(),
        "not_ready",
        "a redirect says nothing about the PDF"
    );
    assert!(
        error.retryable(),
        "fixing the address must be enough to recover the file"
    );
}

#[test]
fn health_reads_minerus_own_status_word() {
    let healthy = FakeServer::start(|_| {
        (
            200,
            json!({ "status": "healthy", "version": "3.4.5" })
                .to_string()
                .into_bytes(),
        )
    });
    assert_eq!(probe(&healthy.origin()), LocalHealth::Ready);
    assert!(MinerULocal::new(&healthy.origin()).health().ready);

    let starting = FakeServer::start(|_| {
        (
            503,
            json!({ "status": "unhealthy" }).to_string().into_bytes(),
        )
    });
    assert_eq!(probe(&starting.origin()), LocalHealth::NotServing);

    assert_eq!(probe(&dead_origin()), LocalHealth::Unreachable);
}

#[test]
fn a_v1_server_is_named_rather_than_called_unreachable() {
    let v1 = FakeServer::start(|hit| {
        if hit.url == V1_HEALTH_PATH {
            (200, json!({ "status": "ok" }).to_string().into_bytes())
        } else {
            (404, Vec::new())
        }
    });
    assert_eq!(probe(&v1.origin()), LocalHealth::WrongApi);
    assert!(!MinerULocal::new(&v1.origin()).health().ready);
}

/// `PARSE_GATE`. Not `FakeServer`, which serializes requests itself and would
/// pass with the gate deleted: this listener serves each connection on its
/// own thread and counts how many are in flight together.
#[test]
fn a_second_parse_waits_for_the_first() {
    use std::io::BufRead;
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let peak = Arc::new(AtomicU64::new(0));
    let live = Arc::new(AtomicU64::new(0));

    let served = {
        let (peak, live) = (peak.clone(), live.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let (peak, live) = (peak.clone(), live.clone());
                std::thread::spawn(move || {
                    let mut stream = stream.unwrap();
                    let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                    let mut length = 0usize;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                            break;
                        }
                        if let Some(value) = line
                            .to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(str::trim)
                            .and_then(|v| v.parse::<usize>().ok())
                        {
                            length = value;
                        }
                    }
                    std::io::copy(
                        &mut reader.by_ref().take(length as u64),
                        &mut std::io::sink(),
                    )
                    .unwrap();

                    // In flight from here to the response.
                    let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(150));
                    live.fetch_sub(1, Ordering::SeqCst);

                    let body = result_zip("Doc", json!([]));
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    stream.write_all(head.as_bytes()).unwrap();
                    stream.write_all(&body).unwrap();
                    stream.flush().ok();
                });
            }
        })
    };

    let origin = format!("http://127.0.0.1:{port}");
    let dir = Scratch::new("local-queue");
    let threads: Vec<_> = (0..2)
        .map(|n| {
            let (origin, root) = (origin.clone(), dir.to_path_buf());
            std::thread::spawn(move || {
                let pdf = root.join(format!("Doc{n}.pdf"));
                write_pdf(&pdf, 1);
                let images = root.join(format!("images{n}"));
                MinerULocal::new(&origin)
                    .parse(&pdf, &images, "images", &|_| {})
                    .unwrap();
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    served.join().ok();

    assert_eq!(
        peak.load(Ordering::SeqCst),
        1,
        "both parses were in flight at once"
    );
}

/// A file waiting for the gate has reported nothing, so it still reads as
/// queued rather than "Parsing 0/N" behind another file.
#[test]
fn a_waiting_parse_reports_no_progress_until_it_has_the_gate() {
    let dir = Scratch::new("local-gate-progress");
    let pdf = dir.join("Doc.pdf");
    write_pdf(&pdf, 2);
    let zip = result_zip("Doc", json!([]));
    let server = FakeServer::start(move |_| (200, zip.clone()));

    let seen = Arc::new(Mutex::new(Vec::new()));
    let held = PARSE_GATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let parse = {
        let (origin, seen, root) = (server.origin(), seen.clone(), dir.to_path_buf());
        std::thread::spawn(move || {
            MinerULocal::new(&origin)
                .parse(
                    &root.join("Doc.pdf"),
                    &root.join("images"),
                    "images",
                    &|progress| {
                        seen.lock().unwrap().push(progress.pages_done);
                    },
                )
                .unwrap();
        })
    };
    std::thread::sleep(Duration::from_millis(200));
    assert!(seen.lock().unwrap().is_empty(), "progress before the gate");
    drop(held);
    parse.join().unwrap();
    assert_eq!(*seen.lock().unwrap(), vec![0, 2]);
}

/// A PDF that actually says something, for a server that actually reads it.
/// One text object per line, so each `Td` is absolute rather than stacking.
fn write_text_pdf(path: &Path, lines: &[&str]) {
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let font = document.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let resources = document.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font },
    });

    let mut operations = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        operations.push(Operation::new("BT", vec![]));
        operations.push(Operation::new("Tf", vec!["F1".into(), 28.into()]));
        operations.push(Operation::new(
            "Td",
            vec![72.into(), (700 - 60 * n as i64).into()],
        ));
        operations.push(Operation::new("Tj", vec![Object::string_literal(*line)]));
        operations.push(Operation::new("ET", vec![]));
    }
    let content = Content { operations };
    let stream = document.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => stream,
        "Resources" => resources,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
        }),
    );
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    document.trailer.set("Root", catalog);
    document.save(path).unwrap();
}

/// Against a real MinerU: the only check that `FIELDS` are spelled the
/// way that server reads them. Ignored; needs a server:
///
/// ```text
/// uv tool install -U "mineru[core]>=3.4,<4"
/// MINERU_API_OUTPUT_ROOT="$HOME/.cache/mineru-api" \
///   mineru-api --host 127.0.0.1 --port 8000
/// cargo test --lib parse::mineru::local::tests::a_real_mineru -- --ignored --nocapture
/// ```
///
/// `OCULUS_MINERU_URL` moves the address; `OCULUS_MINERU_PDF` swaps in a
/// real document; `OCULUS_MINERU_DUMP` writes the markdown and images out.
#[test]
#[ignore = "needs a MinerU 3.x server on 127.0.0.1:8000 — see the doc comment"]
fn a_real_mineru_answers_the_way_this_client_expects() {
    let base = std::env::var("OCULUS_MINERU_URL")
        .unwrap_or_else(|_| crate::parse::LOCAL_BASE_URL.to_string());
    assert_eq!(
        probe(&base),
        LocalHealth::Ready,
        "no healthy MinerU at {base}"
    );

    let dir = Scratch::new("local-live");
    let pdf = match std::env::var("OCULUS_MINERU_PDF") {
        Ok(path) => PathBuf::from(path),
        Err(_) => {
            let path = dir.join("Live.pdf");
            write_text_pdf(
                &path,
                &[
                    "Chapter One",
                    "The quick brown fox",
                    "jumps over the lazy dog.",
                ],
            );
            path
        }
    };

    let images = dir.join("Live_images");
    let output = MinerULocal::new(&base)
        .parse(&pdf, &images, "Live_images", &|progress| {
            eprintln!("  {}/{} pages", progress.pages_done, progress.total_pages);
        })
        .expect("the parse failed");

    assert_eq!(output.backend.as_deref(), Some(BACKEND));
    assert_eq!(output.parser_version, PARSER_VERSION);
    assert_eq!(output.pages.len(), output.page_count as usize);
    assert!(output.page_count > 0, "no pages");
    // At least one page with text proves the form fields landed.
    let written = output
        .pages
        .iter()
        .filter(|p| !p.markdown.trim().is_empty())
        .count();
    assert!(
        written > 0,
        "every page came back empty — check the form fields"
    );
    eprintln!(
        "{} pages, {written} with markdown, {} images",
        output.page_count, output.image_count
    );

    if let Ok(into) = std::env::var("OCULUS_MINERU_DUMP") {
        let into = PathBuf::from(into);
        fs::create_dir_all(&into).unwrap();
        for page in &output.pages {
            fs::write(
                into.join(format!("page-{:03}.md", page.page_no)),
                &page.markdown,
            )
            .unwrap();
        }
        if images.is_dir() {
            let copied = into.join("Live_images");
            fs::create_dir_all(&copied).unwrap();
            for entry in fs::read_dir(&images).unwrap().flatten() {
                fs::copy(entry.path(), copied.join(entry.file_name())).unwrap();
            }
        }
        eprintln!("dumped to {}", into.display());
    }
}

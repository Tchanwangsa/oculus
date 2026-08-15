//! Headless end-to-end check of the retrieval pipeline.
//!
//! Runs the same `retrieval::{ingest, search, stats}` the Tauri commands call,
//! against a real SQLite file, with the sidecar running. Exists because the
//! commands themselves need an AppHandle, which would otherwise mean the only
//! way to test the pipeline is to click through the app.
//!
//!   cargo run --bin retrieval_smoke -- <db_path> ingest <file_id> <pdf_path>
//!   cargo run --bin retrieval_smoke -- <db_path> search "<query>" [limit]
//!   cargo run --bin retrieval_smoke -- <db_path> stats

use std::path::PathBuf;

use app_lib::retrieval;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: retrieval_smoke <db> <ingest|search|stats> [...]");
        std::process::exit(2);
    }
    let db = PathBuf::from(&args[1]);

    match args[2].as_str() {
        "ingest" => {
            let file_id: i64 = args[3].parse().expect("file_id");
            let pdf = args[4].clone();
            let force = args.get(5).map(|s| s == "force").unwrap_or(false);
            match retrieval::ingest(&db, file_id, pdf, force).await {
                Ok(s) => println!(
                    "ingest ok: file_id={} pages={} with_markdown={} dim={} model={} skipped={}",
                    s.file_id, s.pages_embedded, s.pages_with_markdown, s.dim, s.model, s.skipped
                ),
                Err(e) => {
                    eprintln!("ingest FAILED: {e}");
                    std::process::exit(1);
                }
            }
        }
        "search" => {
            let q = args[3].clone();
            let limit: i64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(5);
            let t = std::time::Instant::now();
            match retrieval::search(&db, q.clone(), limit, None).await {
                Ok(hits) => {
                    println!("query: {q:?}  ({} hits in {:?})", hits.len(), t.elapsed());
                    for h in hits {
                        let peek: String = h
                            .markdown
                            .split_whitespace()
                            .take(14)
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!(
                            "  {:.4}  {} p{:<3} | {}",
                            h.score, h.filename, h.page_no, peek
                        );
                    }
                }
                Err(e) => {
                    eprintln!("search FAILED: {e}");
                    std::process::exit(1);
                }
            }
        }
        "stats" => match retrieval::stats(&db).await {
            Ok(s) => println!(
                "files={} pages={} with_markdown={} model={:?} dim={:?}",
                s.files_embedded, s.pages_embedded, s.pages_with_markdown, s.model, s.dim
            ),
            Err(e) => {
                eprintln!("stats FAILED: {e}");
                std::process::exit(1);
            }
        },
        other => {
            eprintln!("unknown command: {other}");
            std::process::exit(2);
        }
    }
}

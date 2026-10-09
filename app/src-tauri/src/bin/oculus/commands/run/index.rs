//! `oculus index`: parse each PDF and fold it into the retrieval index.

use crate::*;

impl Ctx {
    /// Parse each PDF and fold it into the retrieval index.
    ///
    /// Serial, so the log stays readable; both halves are idempotent. Each file
    /// blocks for minutes and there is deliberately no deadline here — see
    /// `docs/parsing.md`. The two callbacks below
    /// keep a live counter instead. `reparse` parses again any file whose
    /// record predates `PARSER_VERSION`.
    pub(super) fn index_pdfs(
        &self,
        pool: &SqlitePool,
        pdfs: &[(i64, String)],
        embed: bool,
        reparse: bool,
    ) -> Result<(), String> {
        if pdfs.is_empty() {
            return Ok(());
        }
        println!();
        println!(
            "{} {} PDF(s)",
            paint(if embed { "indexing" } else { "parsing" }, BOLD),
            pdfs.len()
        );

        let mut pages_total = 0usize;
        let mut failed = 0usize;

        let mut missing = 0usize;

        for (subject_id, rel) in pdfs {
            // For Office documents the parse/embed target is the derived
            // sibling PDF, not the library file itself.
            let Some(pdf_rel) = app_lib::library::paths::doc_pdf_rel(rel) else {
                continue;
            };
            // Rows can outlive their file (a course renamed, a library moved).
            if !self.data_dir.join(&pdf_rel).is_file() {
                missing += 1;
                continue;
            }
            let name = rel.rsplit('/').next().unwrap_or(rel);
            let label = format!("  {:<52} ", truncate(name, 52));
            print!("{label}");
            let _ = std::io::stdout().flush();

            // Rewrite the line in place; `\x1b[K` clears the longer previous count.
            let outcome = app_lib::sync::parse_pdf_reporting(
                &self.data_dir,
                rel,
                *subject_id,
                reparse,
                &|p| {
                    let mb = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
                    let seen = match (p.phase, p.total_pages) {
                        (app_lib::parse::Phase::UploadWait, _) => {
                            format!("waiting to upload {:.1} MB", mb(p.bytes_total))
                        }
                        (app_lib::parse::Phase::Uploading, _) => format!(
                            "uploading {:.1}/{:.1} MB",
                            mb(p.bytes_done),
                            mb(p.bytes_total)
                        ),
                        (_, 0) => format!("{} pages", p.pages_done),
                        (_, total) => format!("{}/{total} pages", p.pages_done),
                    };
                    print!("\r{label}{}\x1b[K", paint(&seen, DIM));
                    let _ = std::io::stdout().flush();
                },
            );
            print!("\r{label}\x1b[K");
            let parsed = match outcome {
                Ok(summary) => summary.to_string(),
                Err(e) => {
                    println!("{}", paint(&e.to_string(), RED));
                    failed += 1;
                    continue;
                }
            };
            print!("{}", paint(&parsed, DIM));
            let _ = std::io::stdout().flush();

            if !embed {
                println!();
                continue;
            }

            // Everything printed so far on this line, so the embed's counter can rewrite
            // in place after it: a slow embed must not look like a hang.
            let stem = format!("{label}{}  ", paint(&parsed, DIM));
            let outcome = self.rt.block_on(async {
                let Some(file_id) = store::file_id(pool, *subject_id, rel).await? else {
                    return Err("not in the database".to_string());
                };
                let abs = self.data_dir.join(&pdf_rel).to_string_lossy().to_string();
                let line = stem.clone();
                app_lib::pages::retrieval::ingest_reporting(
                    &app_lib::library::paths::db_path(&self.data_dir),
                    file_id,
                    abs,
                    false,
                    std::sync::Arc::new(move |p: app_lib::embed::Progress| {
                        let seen = match p.total_pages {
                            0 => format!("embedding {} pages", p.pages_done),
                            total => format!("embedding {}/{total} pages", p.pages_done),
                        };
                        print!("\r{line}{}\x1b[K", paint(&seen, DIM));
                        let _ = std::io::stdout().flush();
                    }),
                )
                .await
                // The kind is for the app's pipeline row; a terminal prints the sentence.
                .map_err(|e| e.message)
            });
            print!("\r{stem}\x1b[K");

            match outcome {
                Ok(s) => {
                    pages_total += s.pages_embedded;
                    println!(
                        "{}",
                        paint(
                            &format!(
                                "{} pages, {} with text",
                                s.pages_embedded, s.pages_with_markdown
                            ),
                            DIM
                        )
                    );
                }
                Err(e) => {
                    println!("{}", paint(&e, RED));
                    failed += 1;
                }
            }
        }

        self.rt
            .block_on(store::reconcile_parse_status(pool, &self.data_dir))
            .ok();

        println!();
        if embed {
            println!("{} {pages_total} page(s) indexed", paint("done", GREEN));
        }
        if failed > 0 {
            println!("{} {failed} PDF(s) failed", paint("warning", YELLOW));
        }
        if missing > 0 {
            println!(
                "{} {missing} database row(s) point at files no longer on disk",
                paint("warning", YELLOW)
            );
        }
        Ok(())
    }

    /// Re-run parse and embed over PDFs already on record, without downloading.
    pub(crate) fn index(&self, args: &IndexArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the index lives in the database")?;
        let ids = self.subject_ids(&pool, &args.codes)?;

        let pdfs = self.rt.block_on(store::pdf_files(&pool, &ids))?;
        if pdfs.is_empty() {
            println!("{}", paint("no PDFs on record — run a sync first", DIM));
            return Ok(());
        }
        if args.reparse {
            let outdated = pdfs
                .iter()
                .filter_map(|(_, rel)| app_lib::library::paths::doc_pdf_rel(rel))
                .filter(|pdf_rel| app_lib::parse::is_outdated(&self.data_dir.join(pdf_rel)))
                .count();
            println!(
                "{} {outdated} of {} PDF(s) parsed before parser version {}",
                paint("re-parsing", BOLD),
                pdfs.len(),
                app_lib::parse::PARSER_VERSION
            );
        }
        self.index_pdfs(&pool, &pdfs, true, args.reparse)
    }
}

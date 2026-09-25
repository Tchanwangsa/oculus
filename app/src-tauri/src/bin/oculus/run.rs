//! `oculus list`, `oculus run` and `oculus index`.

use super::*;

impl Ctx {
    // ── list ─────────────────────────────────────────────────────────────────

    pub(crate) fn list(&self, args: ListArgs) -> Result<(), String> {
        if args.lectures {
            return self.list_lectures(&args.codes);
        }
        self.list_subjects(args.refresh)
    }

    fn list_subjects(&self, refresh: bool) -> Result<(), String> {
        let pool = self.db();

        // Fetch live when asked, or when the database is empty.
        let mut rows = match &pool {
            Some(p) => self.rt.block_on(store::subjects(p))?,
            None => Vec::new(),
        };
        if refresh || rows.is_empty() {
            let engine = self.engine(false);
            if !engine.canvas.has_session() {
                return Err("not connected — run `oculus auth login`".to_string());
            }
            let courses = engine.list_courses()?;
            if let Some(p) = &pool {
                self.rt.block_on(store::upsert_subjects(p, &courses))?;
                rows = self.rt.block_on(store::subjects(p))?;
            } else {
                for c in &courses {
                    println!(
                        "{:<24} {:<12} {}",
                        c.code,
                        c.term.clone().unwrap_or_default(),
                        c.name
                    );
                }
                return Ok(());
            }
        }

        if self.json {
            #[derive(Serialize)]
            struct Subject<'a> {
                id: i64,
                code: &'a str,
                name: &'a str,
                term: Option<&'a str>,
                current: bool,
                selected: bool,
                last_synced_at: Option<&'a str>,
            }
            let out: Vec<Subject> = rows
                .iter()
                .map(|s| Subject {
                    id: s.id,
                    code: &s.code,
                    name: &s.name,
                    term: s.term_name.as_deref(),
                    current: s.is_current,
                    selected: s.selected,
                    last_synced_at: s.last_synced_at.as_deref(),
                })
                .collect();
            return self.emit(&out);
        }

        if rows.is_empty() {
            println!("{}", paint("no subjects", DIM));
            return Ok(());
        }
        for s in &rows {
            let mark = if s.is_current { paint("●", GREEN) } else { paint("○", DIM) };
            let synced = s
                .last_synced_at
                .clone()
                .map(|t| t.chars().take(10).collect::<String>())
                .unwrap_or_else(|| "never".into());
            println!(
                "{mark} {:<24} {:<14} {} {}",
                s.code,
                s.term_name.clone().unwrap_or_default(),
                paint(&format!("{synced:<11}"), DIM),
                s.name
            );
        }
        Ok(())
    }

    fn list_lectures(&self, codes: &[String]) -> Result<(), String> {
        let pool = self.db().ok_or("lectures live in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let wanted = filter_subjects(&subjects, codes, true)?;

        if self.json {
            #[derive(Serialize)]
            struct Lecture<'a> {
                id: &'a str,
                subject: &'a str,
                title: &'a str,
                date: &'a str,
                duration_seconds: i64,
                has_video: bool,
                has_transcript: bool,
            }
            let mut out: Vec<Lecture> = Vec::new();
            let rows: Vec<(String, Vec<store::LectureRow>)> = wanted
                .iter()
                .map(|s| Ok((s.code.clone(), self.rt.block_on(store::lectures(&pool, s.id))?)))
                .collect::<Result<_, String>>()?;
            for (code, lectures) in &rows {
                out.extend(lectures.iter().map(|l| Lecture {
                    id: &l.id,
                    subject: code,
                    title: &l.title,
                    date: &l.date,
                    duration_seconds: l.duration_seconds,
                    has_video: l.has_video,
                    has_transcript: l.has_transcript,
                }));
            }
            return self.emit(&out);
        }

        for s in &wanted {
            let rows = self.rt.block_on(store::lectures(&pool, s.id))?;
            println!("{}  {}", paint(&s.code, BOLD), paint(&format!("{} lectures", rows.len()), DIM));
            for l in &rows {
                let mins = l.duration_seconds / 60;
                let marks = format!(
                    "{}{}",
                    if l.has_video { "video" } else { "     " },
                    if l.has_transcript { " transcript" } else { "" }
                );
                // The id leads, dimmed: its first eight characters are what `oculus lecture`
                // takes as a prefix.
                println!(
                    "  {} {:<11} {mins:>4}m  {} {}",
                    paint(&l.id.chars().take(8).collect::<String>(), DIM),
                    l.date.chars().take(10).collect::<String>(),
                    paint(&format!("{marks:<22}"), DIM),
                    l.title
                );
            }
        }
        Ok(())
    }

    // ── run ──────────────────────────────────────────────────────────────────

    pub(crate) fn run(&self, args: RunArgs) -> Result<(), String> {
        if args.lectures {
            return self.run_lectures(&args);
        }
        self.run_subjects(&args)
    }

    /// Sync the Echo360 lecture list, optionally pulling media; one LTI launch per
    /// subject, since the Echo360 session is per course.
    fn run_lectures(&self, args: &RunArgs) -> Result<(), String> {
        let pool = self.db().ok_or("lectures are stored in the database")?;
        let cookie = std::fs::read_to_string(app_lib::paths::cookie_path(&self.data_dir))
            .map_err(|_| "not connected — run `oculus auth login`".to_string())?;

        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let wanted = filter_subjects(&subjects, &args.codes, !args.all)?;
        if wanted.is_empty() {
            return Err("no subjects to sync — pass subject codes or --all".to_string());
        }

        let ffmpeg = (args.videos)
            .then(|| app_lib::echo360::find_ffmpeg(None))
            .flatten();
        if args.videos && ffmpeg.is_none() {
            return Err("ffmpeg not found — run `bun run ffmpeg` in app/".to_string());
        }

        for s in &wanted {
            println!("{}", paint(&s.code, BOLD));
            let session = match app_lib::echo360::connect(cookie.trim(), s.id) {
                Ok(sess) => sess,
                Err(e) => {
                    eprintln!("  {} {e}", paint("skip", YELLOW));
                    continue;
                }
            };
            let lectures = app_lib::echo360::syllabus(&session)?;
            self.rt.block_on(store::upsert_lectures(&pool, s.id, &lectures))?;
            println!("  {} lecture(s)", lectures.len());

            for l in &lectures {
                let dir = app_lib::echo360::lecture_dir(&self.data_dir, &l.id);
                let date = l.date.chars().take(10).collect::<String>();

                if args.transcripts {
                    let path = dir.join("transcript.vtt");
                    if !path.exists() {
                        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                        match app_lib::echo360::transcript(&session, &l.lesson_id, &l.id) {
                            Ok(vtt) => {
                                std::fs::write(&path, vtt).map_err(|e| e.to_string())?;
                                self.rt.block_on(store::set_lecture_path(
                                    &pool, &l.id, "transcript_path", &path.to_string_lossy(),
                                ))?;
                                println!("  {}  {date}  {}", paint("transcript", DIM), l.title);
                            }
                            Err(e) => eprintln!("  {} {}: {e}", paint("warn", YELLOW), l.title),
                        }
                    }
                }

                if let Some(ffmpeg) = &ffmpeg {
                    // Source 1 only (the Presenter screen); the app fetches room cameras on demand.
                    let final_ = app_lib::echo360::source_path(&dir, 1);
                    if final_.exists() {
                        continue;
                    }
                    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    let raw = app_lib::echo360::partial_path(&dir, 1);
                    print!("  {}       {date}  {} ", paint("video", DIM), l.title);
                    let _ = std::io::stdout().flush();

                    let outcome = app_lib::echo360::download_url(&session, &l.id, &l.lesson_id, 1)
                        // Ctrl-C is the CLI's cancel; nothing here can ask to stop.
                        .and_then(|url| {
                            app_lib::echo360::stream_to_file(&url, &raw, &|_| {}, &|| false)
                        })
                        .and_then(|bytes| {
                            if app_lib::echo360::trim_video(ffmpeg, &raw, &final_) {
                                Ok(bytes)
                            } else {
                                Err("ffmpeg trim failed".to_string())
                            }
                        });
                    std::fs::remove_file(&raw).ok();

                    match outcome {
                        Ok(bytes) => {
                            println!("{}", paint(&human_bytes(bytes), DIM));
                            self.rt.block_on(store::set_lecture_path(
                                &pool, &l.id, "video_path", &final_.to_string_lossy(),
                            ))?;
                        }
                        Err(e) => {
                            std::fs::remove_file(&final_).ok();
                            println!("{}", paint(&e, RED));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn run_subjects(&self, args: &RunArgs) -> Result<(), String> {
        // No in-scrape parse: `index_pdfs` below parses the same files serially, and
        // both at once would parse one PDF twice, concurrently.
        let engine = self.engine(false);
        if !engine.canvas.has_session() {
            return Err("not connected — run `oculus auth login`".to_string());
        }
        let who = engine.canvas.whoami()?;

        let pool = self.db();

        // Refresh courses first so a subject added this week matches a code filter.
        let courses = engine.list_courses()?;
        if let Some(p) = &pool {
            self.rt.block_on(store::upsert_subjects(p, &courses))?;
        }

        let targets: Vec<sync::Subject> = match (&pool, args.codes.is_empty()) {
            // No filter and a database: honour the app's subject selection.
            (Some(p), true) => {
                let rows = self.rt.block_on(store::subjects(p))?;
                rows.iter()
                    .filter(|s| s.selected && (args.all || s.is_current))
                    .map(|s| sync::Subject { id: s.id, code: s.code.clone() })
                    .collect()
            }
            _ => courses
                .iter()
                .filter(|c| {
                    if args.codes.is_empty() {
                        args.all || c.is_current
                    } else {
                        args.codes.iter().any(|w| matches_code(&c.code, w))
                    }
                })
                .map(|c| sync::Subject { id: c.id, code: c.code.clone() })
                .collect(),
        };

        if targets.is_empty() {
            return Err(if args.codes.is_empty() {
                "no current subjects selected — pass subject codes or --all".to_string()
            } else {
                format!("no subject matched {}", args.codes.join(", "))
            });
        }

        println!("{} as {who}", paint("canvas", DIM));
        println!("syncing {} subject(s): {}", targets.len(), targets.iter().map(|s| s.code.as_str()).collect::<Vec<_>>().join(", "));
        println!();

        let target_codes: Vec<String> = targets.iter().map(|s| s.code.clone()).collect();
        let run_id = pool
            .as_ref()
            .and_then(|p| self.rt.block_on(store::start_run(p, &target_codes)).ok());

        // Print a line per artifact and upsert the rows the app's listener would.
        let reporter = TermReporter::new();
        let sink = reporter.sink();
        let engine = Engine::new(&self.data_dir, Box::new(reporter)).with_pdf_parsing(false);

        let started = std::time::Instant::now();
        let done = engine.scrape(&targets);

        let written = sink.lock().unwrap().clone();
        if let Some(p) = &pool {
            self.rt.block_on(async {
                for f in &written {
                    if let Err(e) = store::upsert_file(p, f.subject_id, &f.relative_path, f.size_bytes, &f.category, f.canvas_id, f.source_url.as_deref(), f.action != "unchanged").await {
                        eprintln!("{} {e}", paint("db:", YELLOW));
                    }
                }
                if let Some(id) = run_id {
                    store::finish_run(p, id, "completed", done, None).await.ok();
                    store::add_log(p, "info", &format!("CLI synced {done} subject(s)"), Some(id)).await.ok();
                }
            });
        }

        println!();
        println!(
            "{} {done} subject(s), {} artifact(s) in {:.1}s",
            paint("done", GREEN),
            written.len(),
            started.elapsed().as_secs_f64()
        );

        // Class times and due dates: Canvas's calendar API, database only.
        if let Some(p) = &pool {
            let mut total = 0usize;
            for t in &targets {
                match app_lib::calendar::fetch(&engine.canvas, t.id) {
                    Ok(events) => {
                        total += events.len();
                        if let Err(e) =
                            self.rt.block_on(store::replace_calendar_events(p, t.id, &events))
                        {
                            eprintln!("{} {e}", paint("calendar:", YELLOW));
                        }
                    }
                    Err(e) => eprintln!("{} {}: {e}", paint("calendar:", YELLOW), t.code),
                }
            }
            println!("{} {total} calendar event(s)", paint("calendar", DIM));
        }

        if args.no_parse {
            return Ok(());
        }
        let Some(p) = &pool else {
            println!("{}", paint("no database — skipping parse and index", YELLOW));
            return Ok(());
        };

        let pdfs: Vec<(i64, String)> = written
            .iter()
            .filter(|f| f.relative_path.to_lowercase().ends_with(".pdf"))
            .map(|f| (f.subject_id, f.relative_path.clone()))
            .collect();
        self.index_pdfs(p, &pdfs, !args.no_embed)
    }

    // ── Parse + embed ────────────────────────────────────────────────────────

    /// Parse each PDF and fold it into the retrieval index.
    ///
    /// Serial, so the log stays readable; both halves are idempotent. Each file
    /// blocks for minutes and there is deliberately no deadline here — see
    /// CLAUDE.md "A parse or an embed blocks for minutes". The two callbacks below
    /// keep a live counter instead.
    fn index_pdfs(&self, pool: &SqlitePool, pdfs: &[(i64, String)], embed: bool) -> Result<(), String> {
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
            let Some(pdf_rel) = app_lib::paths::doc_pdf_rel(rel) else {
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
            let outcome =
                app_lib::sync::parse_pdf_reporting(&self.data_dir, rel, *subject_id, &|p| {
                    let seen = match p.total_pages {
                        0 => format!("{} pages", p.pages_done),
                        total => format!("{}/{total} pages", p.pages_done),
                    };
                    print!("\r{label}{}\x1b[K", paint(&seen, DIM));
                    let _ = std::io::stdout().flush();
                });
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
                app_lib::retrieval::ingest_reporting(
                    &app_lib::paths::db_path(&self.data_dir),
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
                        paint(&format!("{} pages, {} with text", s.pages_embedded, s.pages_with_markdown), DIM)
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
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let wanted = filter_subjects(&subjects, &args.codes, false)?;
        let ids: Vec<i64> = wanted.iter().map(|s| s.id).collect();

        let pdfs = self.rt.block_on(store::pdf_files(&pool, &ids))?;
        if pdfs.is_empty() {
            println!("{}", paint("no PDFs on record — run a sync first", DIM));
            return Ok(());
        }
        self.index_pdfs(&pool, &pdfs, true)
    }
}

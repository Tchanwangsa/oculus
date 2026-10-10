//! `oculus status`.

use super::*;

impl Ctx {
    pub(crate) fn status(&self) -> Result<(), String> {
        #[derive(Serialize)]
        struct Service {
            connected: bool,
            user: Option<String>,
            /// Why not, when `connected` is false.
            detail: Option<String>,
        }
        /// The configured parse backend's own `Health`. `parser_version` is the
        /// version handshake for artifacts this binary reads.
        #[derive(Serialize)]
        struct ParserStatus {
            backend: String,
            ready: bool,
            parser_version: Option<u32>,
            detail: Option<String>,
        }
        #[derive(Serialize)]
        struct Counts {
            total: usize,
            current: usize,
            synced: usize,
        }
        #[derive(Serialize)]
        struct FileCounts {
            total: i64,
            parsed: i64,
        }
        fn describe(s: &Service) -> String {
            match (&s.user, &s.detail) {
                (Some(name), _) => format!("{} as {name}", paint("connected", GREEN)),
                (None, Some(why)) => paint(why, YELLOW),
                (None, None) => paint("unknown", DIM),
            }
        }

        let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
        let canvas_status = match canvas.whoami() {
            Ok(name) => Service {
                connected: true,
                user: Some(name),
                detail: None,
            },
            Err(_) if !canvas.has_session() => Service {
                connected: false,
                user: None,
                detail: Some("signed out — run `oculus auth login`".to_string()),
            },
            Err(e) => Service {
                connected: false,
                user: None,
                detail: Some(e),
            },
        };

        let ed = app_lib::ed::Ed::open(&self.data_dir);
        let ed_status = if !ed.has_session() {
            Service {
                connected: false,
                user: None,
                detail: Some("not connected — connects automatically on the next sync".to_string()),
            }
        } else {
            match ed.whoami() {
                Ok(name) => Service {
                    connected: true,
                    user: Some(name),
                    detail: None,
                },
                Err(e) => Service {
                    connected: false,
                    user: None,
                    detail: Some(e),
                },
            }
        };

        // Local only, no quota; the local engine's `health()` is a loopback connect
        // with its own short timeout.
        let parser = match app_lib::parse::backend() {
            Ok(backend) => match app_lib::parse::preflight(backend.as_ref()) {
                Ok(health) => ParserStatus {
                    backend: health.backend,
                    ready: health.ready,
                    parser_version: Some(health.parser_version),
                    detail: None,
                },
                Err(e) => ParserStatus {
                    backend: backend.health().backend,
                    ready: false,
                    parser_version: Some(backend.health().parser_version),
                    detail: Some(e.to_string()),
                },
            },
            Err(e) => ParserStatus {
                backend: app_lib::parse::parse_config().engine.as_str().to_string(),
                ready: false,
                parser_version: None,
                detail: Some(e.to_string()),
            },
        };

        let pool = self.db();
        let (subjects, files) = match &pool {
            Some(pool) => self.rt.block_on(async {
                let rows = store::subjects(pool).await.unwrap_or_default();
                let counts = Counts {
                    total: rows.len(),
                    current: rows.iter().filter(|s| s.is_current).count(),
                    synced: rows.iter().filter(|s| s.last_synced_at.is_some()).count(),
                };
                let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files")
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);
                let parsed: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE parse_status = 'quality'")
                        .fetch_one(pool)
                        .await
                        .unwrap_or(0);
                (Some(counts), Some(FileCounts { total, parsed }))
            }),
            None => (None, None),
        };

        let index = pool.as_ref().and_then(|_| {
            self.rt
                .block_on(app_lib::retrieval::stats(&app_lib::paths::db_path(
                    &self.data_dir,
                )))
                .ok()
        });

        if self.json {
            #[derive(Serialize)]
            struct Report<'a> {
                data_dir: String,
                canvas: &'a Service,
                ed: &'a Service,
                parser: &'a ParserStatus,
                subjects: &'a Option<Counts>,
                files: &'a Option<FileCounts>,
                index: &'a Option<app_lib::retrieval::IndexStats>,
            }
            return self.emit(&Report {
                data_dir: self.data_dir.display().to_string(),
                canvas: &canvas_status,
                ed: &ed_status,
                parser: &parser,
                subjects: &subjects,
                files: &files,
                index: &index,
            });
        }

        println!("{}  {}", paint("library", DIM), self.data_dir.display());
        println!("{}   {}", paint("canvas", DIM), describe(&canvas_status));
        println!("{}       {}", paint("ed", DIM), describe(&ed_status));
        println!(
            "{}   {}",
            paint("parser", DIM),
            match (&parser.detail, parser.parser_version) {
                (None, Some(v)) => paint(&format!("{} (v{v})", parser.backend), GREEN),
                (Some(why), _) => paint(why, YELLOW),
                (None, None) => paint("unknown", DIM),
            }
        );
        if let Some(c) = &subjects {
            println!(
                "{} {} ({} current, {} synced)",
                paint("subjects", DIM),
                c.total,
                c.current,
                c.synced
            );
        }
        if let Some(f) = &files {
            println!(
                "{}    {} ({} parsed)",
                paint("files", DIM),
                f.total,
                f.parsed
            );
        }
        if let Some(i) = &index {
            println!(
                "{}    {} page(s) across {} file(s){}",
                paint("index", DIM),
                i.pages_embedded,
                i.files_embedded,
                match &i.model {
                    Some(m) => paint(&format!("  {m}"), DIM),
                    None => String::new(),
                }
            );
            // Stored but not searchable: its own line so the searchable count stays plain.
            if i.pages_stale > 0 {
                println!(
                    "{}    {} page(s) need re-embedding{}",
                    paint("stale", DIM),
                    i.pages_stale,
                    match i.stale_models.is_empty() {
                        true => String::new(),
                        false => paint(&format!("  from {}", i.stale_models.join(", ")), DIM),
                    }
                );
            }
        }
        Ok(())
    }
}

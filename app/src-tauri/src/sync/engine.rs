//! The engine: its state, the course list, and the per-course run order.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::agents;
use crate::sources::canvas::Canvas;
use crate::sync::{Course, LinkCrawl, Progress, Reporter, Subject, SyncOptions, TaskDocs};

pub struct Engine {
    pub canvas: Canvas,
    pub ed: crate::sources::ed::Ed,
    pub(super) data_dir: PathBuf,
    pub(super) reporter: Box<dyn Reporter>,
    pub(super) parse_pdfs: bool,
    pub(super) options: SyncOptions,
    /// Canvas file id → (modified_at, size) at last download
    /// (`file-manifest.json`). A matching pair with the artifact on disk skips
    /// the download.
    pub(super) manifest: std::cell::RefCell<HashMap<String, (String, u64)>>,
}

impl Engine {
    pub fn new(data_dir: &Path, reporter: Box<dyn Reporter>) -> Self {
        let manifest = std::fs::read_to_string(data_dir.join("file-manifest.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Engine {
            canvas: Canvas::open(data_dir),
            ed: crate::sources::ed::Ed::open(data_dir),
            data_dir: data_dir.to_path_buf(),
            reporter,
            parse_pdfs: true,
            options: SyncOptions::default(),
            manifest: std::cell::RefCell::new(manifest),
        }
    }

    /// Best-effort; a lost manifest only costs re-downloads, never data.
    fn save_manifest(&self) {
        if let Ok(json) = serde_json::to_string(&*self.manifest.borrow()) {
            let _ = std::fs::write(self.data_dir.join("file-manifest.json"), json);
        }
    }

    pub fn with_pdf_parsing(mut self, on: bool) -> Self {
        self.parse_pdfs = on;
        self
    }

    pub fn with_options(mut self, options: SyncOptions) -> Self {
        self.options = options;
        self
    }

    /// Academic courses only — the term filter drops sandboxes and training
    /// shells.
    pub fn list_courses(&self) -> Result<Vec<Course>, String> {
        const NON_SUBJECT_PREFIXES: &[&str] = &["MPMP"];

        let all = self
            .canvas
            .get_all("/api/v1/courses?per_page=100&include[]=term&include[]=account")?;

        let academic: Vec<&serde_json::Value> = all
            .iter()
            .filter(|c| {
                let term = c["term"]["name"].as_str();
                let state = c["workflow_state"].as_str().unwrap_or("");
                let code = c["course_code"].as_str().unwrap_or("");
                term.is_some_and(|t| t != "Default Term")
                    && (state == "available" || state == "completed")
                    && !NON_SUBJECT_PREFIXES.iter().any(|p| code.starts_with(p))
            })
            .collect();

        // The newest term with live courses is "current". Ranked by
        // `terms::term_key`, never compared as text.
        let latest = academic
            .iter()
            .filter(|c| c["workflow_state"] == "available")
            .filter_map(|c| c["term"]["name"].as_str())
            .max_by_key(|t| crate::sync::terms::term_key(t))
            .map(str::to_string);

        Ok(academic
            .into_iter()
            .map(|c| {
                let term = c["term"]["name"].as_str().map(str::to_string);
                let workflow_state = c["workflow_state"].as_str().unwrap_or("").to_string();
                Course {
                    id: c["id"].as_i64().unwrap_or(0),
                    code: c["course_code"].as_str().unwrap_or("").to_string(),
                    name: c["name"].as_str().unwrap_or("").to_string(),
                    is_current: term.is_some() && term == latest && workflow_state == "available",
                    term,
                    workflow_state,
                }
            })
            .filter(|c| c.id != 0)
            .collect())
    }

    /// A subject that fails is logged and skipped. Returns how many subjects
    /// were attempted.
    pub fn scrape(&self, subjects: &[Subject]) -> usize {
        let total = subjects.len();

        // Before the per-course links below, so they never dangle.
        if let Err(e) = agents::ensure_library_docs(&self.data_dir) {
            self.reporter
                .log("warning", "", &format!("agent docs: {e}"));
        }

        for (i, c) in subjects.iter().enumerate() {
            if self.reporter.cancelled() {
                return i;
            }
            if let Err(e) = self.scrape_course(c, i, total) {
                self.reporter.log("error", &c.code, &e);
            }
            // Idempotent; gives a newly scraped subject its agent scaffold.
            if let Err(e) = agents::link_course(&self.data_dir, &c.code) {
                self.reporter
                    .log("warning", &c.code, &format!("agent docs: {e}"));
            }
            self.save_manifest();
            self.reporter.progress(&Progress {
                done: i + 1,
                total,
                course: c.code.clone(),
                phase: "complete".into(),
                label: String::new(),
            });
        }
        subjects.len()
    }

    fn scrape_course(&self, c: &Subject, idx: usize, total: usize) -> Result<(), String> {
        let phase = |name: &str| {
            self.reporter.progress(&Progress {
                done: idx,
                total,
                course: c.code.clone(),
                phase: name.into(),
                label: String::new(),
            })
        };

        let mut crawl = LinkCrawl::default();

        phase("home");
        if self.reporter.cancelled() {
            return Ok(());
        }
        self.scrape_home(c, &mut crawl)?;

        if self.options.announcements {
            phase("announcements");
            if self.reporter.cancelled() {
                return Ok(());
            }
            self.scrape_announcements(c, &mut crawl)?;
        }

        // A failed assignments fetch must not cost the modules walk; the TOCs
        // fall back to Canvas links.
        let tasks = if self.options.assignments {
            phase("assignments");
            if self.reporter.cancelled() {
                return Ok(());
            }
            self.scrape_assignments(c, &mut crawl).unwrap_or_else(|e| {
                self.reporter
                    .log("warning", &c.code, &format!("assignments: {e}"));
                TaskDocs::default()
            })
        } else {
            TaskDocs::default()
        };

        if self.options.modules {
            phase("modules");
            if self.reporter.cancelled() {
                return Ok(());
            }
            self.scrape_modules(c, &tasks, &mut crawl)?;
        }

        if self.reporter.cancelled() {
            return Ok(());
        }
        self.crawl_links(c, &mut crawl);

        if self.options.ed {
            phase("ed");
            if self.reporter.cancelled() {
                return Ok(());
            }
            if let Err(e) = self.scrape_ed(c) {
                self.reporter.log("warning", &c.code, &format!("ed: {e}"));
            }
        }
        Ok(())
    }
}

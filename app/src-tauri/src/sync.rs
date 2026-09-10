//! The Canvas scrape engine.
//!
//! Ported from `scraper.js`, which ran in a hidden WebView and tunnelled every
//! request and every write back through the local IPC server. The port keeps
//! the same strategy — modules are the driver, and pages and files are fetched
//! through them, so nothing is downloaded twice — and the same on-disk layout.
//!
//! Progress leaves through [`Reporter`] rather than an HTTP POST, so the app
//! forwards it to the frontend as Tauri events and the CLI prints it.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::agents;
use crate::canvas::Canvas;
use crate::md::{self, ImageMap};
use crate::paths;

/// Types stored as-is. Everything downstream — the parsers, the page-image
/// embedder, the viewer — is PDF-shaped; Office formats are stored as
/// themselves plus a derived sibling PDF (see [`OFFICE_TYPES`]).
const DOWNLOADABLE_TYPES: &[&str] = &["application/pdf"];

/// Office formats downloaded and kept as-is, with a LibreOffice-converted PDF
/// written beside them, mapped to the extension the converter needs on its
/// input file.
const OFFICE_TYPES: &[(&str, &str)] = &[
    ("application/vnd.openxmlformats-officedocument.presentationml.presentation", "pptx"),
    ("application/vnd.openxmlformats-officedocument.wordprocessingml.document", "docx"),
    ("application/vnd.ms-powerpoint", "ppt"),
    ("application/msword", "doc"),
];

/// A wedged soffice must not hang the whole sync run.
const OFFICE_CONVERT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

/// What Canvas serves for the file kinds we know how to name; anything larger
/// is skipped rather than filling the disk with lecture recordings.
const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;

const IMAGE_EXT: &[(&str, &str)] = &[
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/jpg", "jpg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("image/svg+xml", "svg"),
    ("image/bmp", "bmp"),
];

// ── Reporting ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub course: String,
    pub phase: String,
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileEvent {
    pub subject_id: i64,
    pub code: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub category: String,
    pub canvas_id: Option<i64>,
    /// The Canvas URL this artifact came from, where links can name it. Set
    /// for pages, whose *URL slug* survives renames while the saved filename
    /// tracks the title — matching a body link to the local copy needs this.
    pub source_url: Option<String>,
    /// `"new"`, `"updated"`, or `"unchanged"` — what this run's write actually
    /// did to the file on disk. Feeds the per-run sync history.
    pub action: &'static str,
}

/// Announced before a course file's bytes start moving, under the same
/// `relative_path` the eventual [`FileEvent`] will carry — this is what lets
/// the UI show "downloading" for a file it has never seen before.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileStart {
    pub subject_id: i64,
    pub code: String,
    pub relative_path: String,
    pub filename: String,
    pub size_bytes: u64,
}

/// Where a run's side effects go. Default methods are no-ops so an embedder
/// only implements what it cares about.
pub trait Reporter: Send + Sync {
    fn progress(&self, _p: &Progress) {}
    fn file_start(&self, _f: &FileStart) {}
    fn file(&self, _f: &FileEvent) {}
    fn log(&self, _level: &str, _course: &str, _message: &str) {}
    /// Checked between items; a run stops at the next boundary once true.
    fn cancelled(&self) -> bool {
        false
    }
}

/// Discards everything. Useful in tests.
pub struct Silent;
impl Reporter for Silent {}

// ── Engine ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Subject {
    pub id: i64,
    pub code: String,
}

/// Where the assignments phase put each document. Module items name
/// assignments and quizzes by `content_id`, so the maps are keyed by Canvas
/// id → course-relative path (`assignments/….md`).
#[derive(Debug, Default)]
pub struct TaskDocs {
    assignments: HashMap<i64, String>,
    quizzes: HashMap<i64, String>,
}

/// The per-course link crawl. Every phase that converts a Canvas HTML body —
/// home/syllabus, announcements, assignment and quiz descriptions, pages —
/// reports the course pages and files that body references here; after the
/// content phases, `crawl_links` drains the stacks depth-first. Fetched pages
/// surface further links (pages nest arbitrarily), the `seen` sets break
/// cycles. This is the single mechanism that guarantees anything reachable
/// from any scraped body lands on disk, no matter which phase found it.
#[derive(Debug, Default)]
struct LinkCrawl {
    seen_pages: HashSet<String>,
    seen_files: HashSet<String>,
    /// Pending page slugs / file ids, popped LIFO.
    pages: Vec<String>,
    files: Vec<String>,
}

impl LinkCrawl {
    /// Queue everything a just-converted body linked to.
    fn absorb(&mut self, (pages, files): (Vec<String>, Vec<String>)) {
        self.pages.extend(pages);
        self.files.extend(files);
    }
}

/// A course as Canvas describes it, plus whether it belongs to the newest term.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Course {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub term: Option<String>,
    pub workflow_state: String,
    pub is_current: bool,
}

impl Course {
    /// The shape the frontend's `upsertSubjects` reads — Canvas's own field
    /// names, plus the current-term marker we computed.
    pub fn to_canvas_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "course_code": self.code,
            "name": self.name,
            "workflow_state": self.workflow_state,
            "term": self.term.as_ref().map(|t| serde_json::json!({ "name": t })),
            "_oculus_is_current": self.is_current,
        })
    }
}

/// Which content categories a sync fetches. Everything is on by default —
/// the app's sync-settings gear persists the user's choice and hands it to
/// `scrape_content` per run; the CLI always syncs everything.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct SyncOptions {
    pub announcements: bool,
    pub assignments: bool,
    pub modules: bool,
    pub ed: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        SyncOptions { announcements: true, assignments: true, modules: true, ed: true }
    }
}

pub struct Engine {
    pub canvas: Canvas,
    /// Ed Discussion, when the user has saved a token; sessionless otherwise,
    /// in which case the ed phase is a no-op.
    pub ed: crate::ed::Ed,
    data_dir: PathBuf,
    reporter: Box<dyn Reporter>,
    /// Port the sidecar posts parse status back to. 0 disables the callback
    /// (the CLI has no IPC server; the parse still runs).
    ipc_port: u16,
    parse_pdfs: bool,
    options: SyncOptions,
    /// Canvas file id → (modified_at, size) at last download, persisted as
    /// `file-manifest.json` in the data dir. When the metadata call reports
    /// the same pair and the artifact is on disk, the download is skipped —
    /// the byte-compare in `write` stays the arbiter whenever we do download.
    manifest: std::cell::RefCell<HashMap<String, (String, u64)>>,
    /// Bounded dispatcher for sidecar parse requests, created on first use.
    /// See `PARSE_WORKERS`.
    parse_queue: std::cell::RefCell<Option<std::sync::mpsc::Sender<ParseJob>>>,
}

impl Engine {
    pub fn new(data_dir: &Path, reporter: Box<dyn Reporter>) -> Self {
        let manifest = std::fs::read_to_string(data_dir.join("file-manifest.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Engine {
            canvas: Canvas::open(data_dir),
            ed: crate::ed::Ed::open(data_dir),
            data_dir: data_dir.to_path_buf(),
            reporter,
            ipc_port: 0,
            parse_pdfs: true,
            options: SyncOptions::default(),
            manifest: std::cell::RefCell::new(manifest),
            parse_queue: std::cell::RefCell::new(None),
        }
    }

    /// Best-effort; a lost manifest only costs re-downloads, never data.
    fn save_manifest(&self) {
        if let Ok(json) = serde_json::to_string(&*self.manifest.borrow()) {
            let _ = std::fs::write(self.data_dir.join("file-manifest.json"), json);
        }
    }

    pub fn with_ipc_port(mut self, port: u16) -> Self {
        self.ipc_port = port;
        self
    }

    pub fn with_pdf_parsing(mut self, on: bool) -> Self {
        self.parse_pdfs = on;
        self
    }

    pub fn with_options(mut self, options: SyncOptions) -> Self {
        self.options = options;
        self
    }

    // ── Course list ──────────────────────────────────────────────────────────

    /// Academic courses only. Canvas hands back sandboxes, orientation shells
    /// and staff training spaces alongside real subjects; the term filter is
    /// what separates them.
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

        // The newest term that still has live courses is "current"; everything
        // else is archive. Term names sort chronologically here.
        let latest = academic
            .iter()
            .filter(|c| c["workflow_state"] == "available")
            .filter_map(|c| c["term"]["name"].as_str())
            .max()
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

    // ── Run ──────────────────────────────────────────────────────────────────

    /// Scrape each subject in turn. A subject that fails is logged and skipped —
    /// one broken course must not cost the user the rest of the run. Returns how
    /// many subjects were attempted.
    pub fn scrape(&self, subjects: &[Subject]) -> usize {
        let total = subjects.len();

        // Write the central `agents/AGENTS.md` before anything points at it,
        // so the per-course links below cannot come out dangling on a library
        // where `oculus docs` has never run.
        if let Err(e) = agents::ensure_library_docs(&self.data_dir) {
            self.reporter.log("warning", "", &format!("agent docs: {e}"));
        }

        for (i, c) in subjects.iter().enumerate() {
            if self.reporter.cancelled() {
                return i;
            }
            if let Err(e) = self.scrape_course(c, i, total) {
                self.reporter.log("error", &c.code, &e);
            }
            // A subject enrolled mid-semester gets its agent scaffold from the
            // run that first scrapes it, rather than waiting for someone to
            // remember `oculus docs`. Idempotent, so every later run is free.
            if let Err(e) = agents::link_course(&self.data_dir, &c.code) {
                self.reporter.log("warning", &c.code, &format!("agent docs: {e}"));
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

        // Collects the pages/files every converted body references; drained
        // depth-first after the content phases so linked content is fetched
        // regardless of which phases were on.
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

        // A failed assignments fetch must not cost the modules walk — the run
        // continues with Canvas links in the TOCs instead of local documents.
        // Switched off, the walk gets the same empty maps.
        let tasks = if self.options.assignments {
            phase("assignments");
            if self.reporter.cancelled() {
                return Ok(());
            }
            self.scrape_assignments(c, &mut crawl).unwrap_or_else(|e| {
                self.reporter.log("warning", &c.code, &format!("assignments: {e}"));
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

    // ── Phase: home + syllabus ───────────────────────────────────────────────

    fn scrape_home(&self, c: &Subject, crawl: &mut LinkCrawl) -> Result<(), String> {
        let course = self
            .canvas
            .get_json(&format!(
                "/api/v1/courses/{}?include[]=syllabus_body&include[]=public_description\
                 &include[]=teachers&include[]=term",
                c.id
            ))
            .unwrap_or(serde_json::Value::Null);

        let name = course["name"].as_str().unwrap_or(&c.code).to_string();
        let term = course["term"]["name"].as_str().unwrap_or("");
        let teachers: Vec<&str> = course["teachers"]
            .as_array()
            .map(|a| a.iter().filter_map(|t| t["display_name"].as_str()).collect())
            .unwrap_or_default();

        let header = |title: &str, extra: &str| {
            let mut meta = Vec::new();
            if !term.is_empty() {
                meta.push(format!("**Term:** {term}"));
            }
            meta.push(format!("**Code:** {}", c.code));
            if !teachers.is_empty() {
                meta.push(format!("**Staff:** {}", teachers.join(", ")));
            }
            format!("# {title}\n\n{}\n{extra}\n", meta.join("  \n"))
        };

        // Saved separately from the front page — a course can have both, and
        // the syllabus is usually the more load-bearing of the two.
        if let Some(syllabus) = course["syllabus_body"].as_str().filter(|s| !s.is_empty()) {
            crawl.absorb(md::canvas_links(syllabus, c.id));
            let body = self.convert(syllabus, c, "syllabus.md");
            let md = format!("{}\n---\n\n{body}", header(&format!("{name} — Syllabus"), ""));
            self.write(c, "syllabus.md", md.as_bytes(), None)?;
        }

        let (body, source) = match self.canvas.get(&format!("/api/v1/courses/{}/front_page", c.id)) {
            Ok(r) if r.ok() => match r.json() {
                Ok(j) => match j["body"].as_str() {
                    Some(b) if !b.is_empty() => (b.to_string(), "Front Page"),
                    _ => (String::new(), ""),
                },
                Err(_) => (String::new(), ""),
            },
            _ => (String::new(), ""),
        };
        let (body, source) = if body.is_empty() {
            match course["public_description"].as_str().filter(|s| !s.is_empty()) {
                Some(d) => (format!("<p>{d}</p>"), "Description"),
                None => (String::new(), ""),
            }
        } else {
            (body, source)
        };

        // No front page and no description: write nothing rather than a stub.
        if body.is_empty() {
            return Ok(());
        }

        crawl.absorb(md::canvas_links(&body, c.id));
        let converted = self.convert(&body, c, "home.md");
        let md = format!(
            "{}\n---\n\n{converted}",
            header(&name, &format!("\n> Source: {source}\n"))
        );
        self.write(c, "home.md", md.as_bytes(), None)?;
        Ok(())
    }

    // ── Phase: announcements ─────────────────────────────────────────────────

    fn scrape_announcements(&self, c: &Subject, crawl: &mut LinkCrawl) -> Result<(), String> {
        let list = self.canvas.get_all(&format!(
            "{}/api/v1/courses/{}/discussion_topics?only_announcements=true&per_page=100&include[]=author",
            crate::canvas::CANVAS_BASE,
            c.id
        ))?;

        for (i, a) in list.iter().enumerate() {
            if self.reporter.cancelled() {
                break;
            }
            let title = a["title"].as_str().unwrap_or("Announcement");
            self.reporter.progress(&Progress {
                done: i + 1,
                total: list.len(),
                course: c.code.clone(),
                phase: "announcements".into(),
                label: title.to_string(),
            });

            let Some(message) = a["message"].as_str().filter(|s| !s.is_empty()) else { continue };
            crawl.absorb(md::canvas_links(message, c.id));
            let date = a["posted_at"]
                .as_str()
                .or_else(|| a["created_at"].as_str())
                .unwrap_or("")
                .chars()
                .take(10)
                .collect::<String>();
            let author = a["author"]["display_name"].as_str().unwrap_or("");

            let mut header = format!("# {title}\n\n");
            if !date.is_empty() {
                header.push_str(&format!("**Date:** {date}  \n"));
            }
            if !author.is_empty() {
                header.push_str(&format!("**From:** {author}\n"));
            }
            header.push_str("\n---\n\n");

            let name = if date.is_empty() {
                format!("announcements/{}.md", slug(title))
            } else {
                format!("announcements/{date}-{}.md", slug(title))
            };
            let md = format!("{header}{}", self.convert(message, c, &name));
            self.write(c, &name, md.as_bytes(), None)?;
        }
        Ok(())
    }

    // ── Phase: assignments + quizzes ─────────────────────────────────────────

    /// Every assignment and quiz, written as `assignments/*.md` and
    /// `quizzes/*.md` with metadata (due date, points) above the converted
    /// description. Returns where each landed, keyed the way module items refer
    /// to them, so the modules phase links locally instead of out to Canvas.
    fn scrape_assignments(&self, c: &Subject, crawl: &mut LinkCrawl) -> Result<TaskDocs, String> {
        let mut docs = TaskDocs::default();
        let mut used_paths: HashSet<String> = HashSet::new();

        let quizzes = self
            .canvas
            .get_all(&format!("/api/v1/courses/{}/quizzes?per_page=100", c.id))?;
        let assignments = self.canvas.get_all(&format!(
            "/api/v1/courses/{}/assignments?per_page=100&include[]=submission",
            c.id
        ))?;
        let total = quizzes.len() + assignments.len();
        let mut done = 0usize;

        // The user's submission state rides on the assignments API; quizzes
        // get theirs through their assignment shell, resolved up front since
        // quiz documents are written first.
        let submitted_quizzes: HashSet<i64> = assignments
            .iter()
            .filter(|a| submission_status(a).is_some())
            .filter_map(|a| a["quiz_id"].as_i64())
            .collect();

        let progress = |label: &str, done: usize| {
            self.reporter.progress(&Progress {
                done,
                total,
                course: c.code.clone(),
                phase: "assignments".into(),
                label: label.to_string(),
            })
        };

        // Classic quizzes first: their assignment shells are skipped below, so
        // the quiz document is the one copy either kind of reference reaches.
        for q in &quizzes {
            if self.reporter.cancelled() {
                return Ok(docs);
            }
            let title = q["title"].as_str().unwrap_or("Quiz");
            done += 1;
            progress(title, done);
            let Some(id) = q["id"].as_i64() else { continue };

            let mut meta = Vec::new();
            push_ts(&mut meta, "Due", q["due_at"].as_str());
            push_ts(&mut meta, "Available until", q["lock_at"].as_str());
            if let Some(p) = q["points_possible"].as_f64() {
                meta.push(format!("**Points:** {}", fmt_points(p)));
            }
            if let Some(n) = q["question_count"].as_i64() {
                meta.push(format!("**Questions:** {n}"));
            }
            if let Some(t) = q["time_limit"].as_f64() {
                meta.push(format!("**Time limit:** {} min", fmt_points(t)));
            }
            match q["allowed_attempts"].as_i64() {
                Some(-1) => meta.push("**Attempts:** unlimited".to_string()),
                Some(n) if n > 1 => meta.push(format!("**Attempts:** {n}")),
                _ => {}
            }
            if submitted_quizzes.contains(&id) {
                meta.push("**Status:** submitted".to_string());
            }

            let path = task_path("quizzes", title, id, &mut used_paths);
            self.write_task_doc(c, &path, title, &meta, q, crawl)?;
            docs.quizzes.insert(id, path.clone());
            // A graded quiz also exists as an assignment; either id reaches it.
            if let Some(aid) = q["assignment_id"].as_i64() {
                docs.assignments.insert(aid, path);
            }
        }

        for a in &assignments {
            if self.reporter.cancelled() {
                return Ok(docs);
            }
            let title = a["name"].as_str().unwrap_or("Assignment");
            done += 1;
            progress(title, done);
            let Some(id) = a["id"].as_i64() else { continue };

            // Classic-quiz shell: the quizzes API already wrote the document.
            let is_quiz = a["submission_types"]
                .as_array()
                .is_some_and(|t| t.iter().any(|s| s == "online_quiz"));
            if is_quiz {
                if let Some(path) = a["quiz_id"].as_i64().and_then(|qid| docs.quizzes.get(&qid)) {
                    docs.assignments.entry(id).or_insert_with(|| path.clone());
                }
                continue;
            }

            let mut meta = Vec::new();
            push_ts(&mut meta, "Due", a["due_at"].as_str());
            push_ts(&mut meta, "Available until", a["lock_at"].as_str());
            if let Some(p) = a["points_possible"].as_f64() {
                meta.push(format!("**Points:** {}", fmt_points(p)));
            }
            if let Some(kinds) = a["submission_types"].as_array() {
                let kinds: Vec<&str> = kinds
                    .iter()
                    .filter_map(|s| s.as_str())
                    .filter(|s| *s != "none" && *s != "not_graded")
                    .collect();
                if !kinds.is_empty() {
                    meta.push(format!("**Submission:** {}", kinds.join(", ").replace('_', " ")));
                }
            }
            if let Some(status) = submission_status(a) {
                meta.push(format!("**Status:** {status}"));
            }

            let path = task_path("assignments", title, id, &mut used_paths);
            self.write_task_doc(c, &path, title, &meta, a, crawl)?;
            docs.assignments.insert(id, path);
        }
        Ok(docs)
    }

    /// Render one assignment/quiz to Markdown and write it. The description's
    /// Canvas links go on the crawl so the link walk fetches them.
    fn write_task_doc(
        &self,
        c: &Subject,
        path: &str,
        title: &str,
        meta: &[String],
        item: &serde_json::Value,
        crawl: &mut LinkCrawl,
    ) -> Result<(), String> {
        let mut md = format!("# {title}\n\n");
        for m in meta {
            md.push_str(m);
            md.push_str("  \n");
        }
        if let Some(url) = item["html_url"].as_str().filter(|u| !u.is_empty()) {
            md.push_str(&format!("[Open in Canvas]({url})\n"));
        }
        md.push_str("\n---\n\n");

        match item["description"].as_str().filter(|s| !s.is_empty()) {
            Some(desc) => {
                crawl.absorb(md::canvas_links(desc, c.id));
                md.push_str(&self.convert(desc, c, path));
            }
            None => md.push_str("_No description._"),
        }
        md.push('\n');
        self.write(c, path, md.as_bytes(), None)?;
        Ok(())
    }

    // ── Phase: Ed Discussion ─────────────────────────────────────────────────

    /// Mirror the subject's Ed Discussion board into `ed/*.md`, one thread per
    /// file including its replies. The Ed session is minted from the Canvas
    /// session via the course's LTI launch whenever the saved token is missing
    /// or dead — and also when the course has an Ed tool the user never opened,
    /// since Ed only creates the enrolment on first launch. A course with no
    /// Ed tool is skipped.
    fn scrape_ed(&self, c: &Subject) -> Result<(), String> {
        let course_id = match self.ed.resolve_course(&self.canvas, c.id, &c.code) {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(()),
            Err(e) => {
                self.reporter.log("info", &c.code, &format!("ed: {e}"));
                return Ok(());
            }
        };
        let threads = self.ed.threads(course_id)?;

        for (i, t) in threads.iter().enumerate() {
            if self.reporter.cancelled() {
                break;
            }
            let title = t["title"].as_str().unwrap_or("Thread");
            self.reporter.progress(&Progress {
                done: i + 1,
                total: threads.len(),
                course: c.code.clone(),
                phase: "ed".into(),
                label: title.to_string(),
            });

            // `number` is the per-course thread number Ed shows in the UI —
            // stable across syncs, so re-runs overwrite instead of duplicating.
            let number = t["number"].as_i64().unwrap_or(0);
            let path = format!("ed/{number:04}-{}.md", slug(title));
            match self.ed.thread_markdown(t) {
                Ok(md) => {
                    self.write(c, &path, md.as_bytes(), None)?;
                }
                Err(e) => self.reporter.log("warning", &c.code, &format!("ed thread {title}: {e}")),
            }
        }
        Ok(())
    }

    // ── Phase: modules (drives pages + files) ────────────────────────────────

    fn scrape_modules(&self, c: &Subject, tasks: &TaskDocs, crawl: &mut LinkCrawl) -> Result<(), String> {
        let modules = self
            .canvas
            .get_all(&format!("/api/v1/courses/{}/modules?include[]=items&per_page=100", c.id))?;

        // Every real module item counts once, even when it triggers nested
        // fetches — otherwise the total moves while the bar is running.
        let total_items: usize = modules
            .iter()
            .map(|m| items_of(m).iter().filter(|it| it["type"] != "SubHeader").count())
            .sum();
        let mut processed = 0usize;

        for m in &modules {
            if self.reporter.cancelled() {
                break;
            }
            let mod_name = m["name"].as_str().unwrap_or("Module");
            let mut toc = vec![format!("# {mod_name}\n")];

            for item in items_of(m) {
                if self.reporter.cancelled() {
                    break;
                }
                let ty = item["type"].as_str().unwrap_or("");
                let title = item["title"].as_str().unwrap_or("Untitled");
                let indent = "  ".repeat(item["indent"].as_u64().unwrap_or(0) as usize);

                if ty == "SubHeader" {
                    toc.push(format!("{indent}## {}", escape_md(title)));
                    continue;
                }

                processed += 1;
                self.reporter.progress(&Progress {
                    done: processed,
                    total: total_items,
                    course: c.code.clone(),
                    phase: "modules".into(),
                    label: title.to_string(),
                });

                match ty {
                    "Page" => {
                        let Some(page_url) = item["page_url"].as_str() else { continue };
                        if crawl.seen_pages.insert(page_url.to_string()) {
                            match self.fetch_page(c, page_url, title) {
                                Ok(Some(links)) => crawl.absorb(links),
                                Ok(None) => {}
                                Err(e) => self.reporter.log("warning", &c.code, &format!("page {page_url}: {e}")),
                            }
                        }
                        toc.push(format!(
                            "{indent}- [{}](../pages/{}.md)",
                            escape_md(title),
                            slug(title)
                        ));
                    }
                    "File" => {
                        let Some(id) = item["content_id"].as_i64() else { continue };
                        let mut saved = None;
                        if crawl.seen_files.insert(id.to_string()) {
                            match self.fetch_file(c, id, Some(title), false) {
                                Ok(p) => saved = p,
                                Err(e) => self.reporter.log("warning", &c.code, &format!("file {id}: {e}")),
                            }
                        }
                        toc.push(match saved {
                            // TOCs live in modules/, so links step up a level.
                            Some(path) => format!(
                                "{indent}- [{}](../{})",
                                escape_md(title),
                                rel_within_course(&path)
                            ),
                            None => format!("{indent}- {} _(file)_", escape_md(title)),
                        });
                    }
                    "Assignment" | "Quiz" => {
                        let kind = if ty == "Quiz" { "quiz" } else { "assignment" };
                        let map = if ty == "Quiz" { &tasks.quizzes } else { &tasks.assignments };
                        let local = item["content_id"].as_i64().and_then(|id| map.get(&id));
                        toc.push(match local {
                            // TOCs live in modules/, so links step up a level.
                            Some(path) => format!(
                                "{indent}- [{}](../{path}) _({kind})_",
                                escape_md(title)
                            ),
                            None => format!(
                                "{indent}- [{}]({}) _({kind})_",
                                escape_md(title),
                                item["html_url"].as_str().unwrap_or("")
                            ),
                        });
                    }
                    "ExternalUrl" => toc.push(format!(
                        "{indent}- [{}]({}) _(external)_",
                        escape_md(title),
                        item["external_url"].as_str().or(item["html_url"].as_str()).unwrap_or("")
                    )),
                    _ => {
                        let url = item["html_url"].as_str().unwrap_or("");
                        toc.push(if url.is_empty() {
                            format!("{indent}- {}", escape_md(title))
                        } else {
                            format!("{indent}- [{}]({url})", escape_md(title))
                        });
                    }
                }
            }

            let pos = m["position"].as_u64().unwrap_or(0);
            let path = format!("modules/{pos:02}-{}.md", slug(mod_name));
            self.write(c, &path, format!("{}\n", toc.join("\n")).as_bytes(), None)?;
        }
        Ok(())
    }

    /// Drain the link crawl: content every scraped body referenced but no
    /// module listed. A DFS over the link graph — each fetched page can
    /// surface further pages and files, which go back on the stacks; the
    /// `seen_*` sets already hold everything the modules walk covered, so
    /// nothing is fetched twice and cycles terminate. No progress emitted —
    /// these are nested extras, not module items.
    fn crawl_links(&self, c: &Subject, crawl: &mut LinkCrawl) {
        while !crawl.pages.is_empty() || !crawl.files.is_empty() {
            if self.reporter.cancelled() {
                break;
            }
            if let Some(page_url) = crawl.pages.pop() {
                if !crawl.seen_pages.insert(page_url.clone()) {
                    continue;
                }
                match self.fetch_page(c, &page_url, &page_url) {
                    Ok(Some(links)) => crawl.absorb(links),
                    Ok(None) => {}
                    Err(e) => self.reporter.log("warning", &c.code, &format!("page {page_url}: {e}")),
                }
                continue;
            }
            let id = crawl.files.pop().expect("loop guard: one stack is non-empty");
            if !crawl.seen_files.insert(id.clone()) {
                continue;
            }
            let Ok(fid) = id.parse::<i64>() else { continue };
            if let Err(e) = self.fetch_file(c, fid, None, false) {
                self.reporter.log("warning", &c.code, &format!("file {id}: {e}"));
            }
        }
    }

    /// Returns the page slugs and file ids this page links to, or `None` if
    /// there was no page body to save.
    #[allow(clippy::type_complexity)]
    fn fetch_page(
        &self,
        c: &Subject,
        page_url: &str,
        title: &str,
    ) -> Result<Option<(Vec<String>, Vec<String>)>, String> {
        let r = self.canvas.get(&format!("/api/v1/courses/{}/pages/{page_url}", c.id))?;
        if !r.ok() {
            return Ok(None);
        }
        let full = r.json()?;
        let Some(body) = full["body"].as_str().filter(|s| !s.is_empty()) else {
            return Ok(None);
        };

        let links = md::canvas_links(body, c.id);
        let page_title = full["title"].as_str().unwrap_or(title);
        let updated = full["updated_at"].as_str().unwrap_or("");

        let out = format!("pages/{}.md", slug(page_title));
        let md_body = format!(
            "# {page_title}\n\n{}---\n\n{}",
            if updated.is_empty() { String::new() } else { format!("_Updated: {updated}_\n\n") },
            self.convert(body, c, &out)
        );
        // Deliberately the *requested* slug, not the canonical `full["url"]`:
        // Canvas keeps resolving a renamed page's old URL, and old body links
        // still use it — this is what lets the app match such a link to the
        // local copy. The canonical slug usually equals `slug(title)`, which
        // the filename already matches.
        let source = format!(
            "{}/courses/{}/pages/{page_url}",
            crate::canvas::CANVAS_BASE,
            c.id
        );
        self.write_from(c, &out, md_body.as_bytes(), None, Some(source))?;
        Ok(Some(links))
    }

    /// Download one Canvas file if its type is allowlisted. Returns the saved
    /// course-relative path.
    fn fetch_file(
        &self,
        c: &Subject,
        file_id: i64,
        display: Option<&str>,
        force: bool,
    ) -> Result<Option<String>, String> {
        let r = self.canvas.get(&format!("/api/v1/files/{file_id}"))?;
        if !r.ok() {
            return Ok(None);
        }
        let info = r.json()?;

        let ct = content_type_of(&info);
        let office = office_ext(&ct);
        if !DOWNLOADABLE_TYPES.contains(&ct.as_str()) && office.is_none() {
            return Ok(None);
        }
        if info["size"].as_u64().unwrap_or(0) > MAX_FILE_BYTES {
            self.reporter.log("warning", &c.code, &format!("file {file_id}: over size cap, skipped"));
            return Ok(None);
        }
        // Staff routinely publish solutions with a release date. Canvas still
        // lists the file and answers the metadata call — it is only the
        // download that is refused — so this has to be checked explicitly, or
        // the run reports a scary auth failure for something entirely normal.
        if info["locked_for_user"].as_bool().unwrap_or(false) {
            let name = info["display_name"].as_str().or(display).unwrap_or("file");
            let until = info["lock_info"]["unlock_at"]
                .as_str()
                .or_else(|| info["unlock_at"].as_str())
                .map(|d| format!(" until {}", &d[..10.min(d.len())]))
                .unwrap_or_default();
            self.reporter.log("info", &c.code, &format!("{name}: locked{until}, skipped"));
            return Ok(None);
        }

        let name = info["filename"]
            .as_str()
            .or_else(|| info["display_name"].as_str())
            .or(display)
            .unwrap_or("file.bin")
            .replace(['/', '\\'], "_");

        // Same version Canvas reported last time, and the artifacts are still
        // on disk (the derived PDF too, for Office files) → skip the download.
        // The metadata call above is the whole cost of an unchanged file.
        let modified = info["modified_at"]
            .as_str()
            .or_else(|| info["updated_at"].as_str())
            .unwrap_or("")
            .to_string();
        let meta_size = info["size"].as_u64().unwrap_or(0);
        if !force && !modified.is_empty() {
            if let Some(rel) = paths::course_rel_path(&c.code, &format!("files/{name}")) {
                let known = self
                    .manifest
                    .borrow()
                    .get(&file_id.to_string())
                    .is_some_and(|(m, s)| *m == modified && *s == meta_size);
                let on_disk = self.data_dir.join(&rel).is_file()
                    && paths::doc_pdf_rel(&rel)
                        .map_or(true, |p| self.data_dir.join(p).is_file());
                if known && on_disk {
                    self.reporter.file(&FileEvent {
                        subject_id: c.id,
                        code: c.code.clone(),
                        relative_path: rel.clone(),
                        size_bytes: meta_size,
                        category: paths::category_from_path(&format!("files/{name}")).to_string(),
                        canvas_id: Some(file_id),
                        source_url: None,
                        action: "unchanged",
                    });
                    return Ok(Some(rel));
                }
            }
        }

        if let Some(rel) = paths::course_rel_path(&c.code, &format!("files/{name}")) {
            self.reporter.file_start(&FileStart {
                subject_id: c.id,
                code: c.code.clone(),
                relative_path: rel,
                filename: name.clone(),
                size_bytes: info["size"].as_u64().unwrap_or(0),
            });
        }

        let Some(url) = self.download_url(&info, file_id)? else { return Ok(None) };
        let bytes = self.fetch_bytes(&url)?;

        // The original is always the library file. Office documents get a
        // *derived* PDF written beside them ("deck.pptx" → "deck.pptx.pdf") —
        // never announced, never a database row — which is what the parser,
        // the embedder and the in-app viewer read.
        let rel = self.write(c, &format!("files/{name}"), &bytes, Some(file_id))?;

        // Only a fully-landed artifact enters the manifest — a failed Office
        // conversion stays out so the next run retries instead of skipping.
        let mut complete = true;
        if let Some(ext) = office {
            match office_to_pdf(&bytes, ext) {
                Ok(pdf) => {
                    paths::write_course_bytes(&self.data_dir, &c.code, &format!("files/{name}.pdf"), &pdf)?;
                    if self.parse_pdfs {
                        self.trigger_parse(&rel, c.id, &c.code);
                    }
                }
                Err(e) => {
                    complete = false;
                    self.reporter.log(
                        "warning",
                        &c.code,
                        &format!("{name}: PDF conversion failed — stored original only ({e})"),
                    );
                }
            }
        }
        if complete && !modified.is_empty() {
            self.manifest
                .borrow_mut()
                .insert(file_id.to_string(), (modified, meta_size));
        }
        Ok(Some(rel))
    }

    /// Re-download a single file on demand — bypasses the unchanged-skip, the
    /// caller explicitly wants fresh bytes. Returns its path relative to the
    /// data directory, or `None` if Canvas will not serve it.
    pub fn refetch_file(&self, c: &Subject, canvas_id: i64) -> Result<Option<String>, String> {
        let rel = self.fetch_file(c, canvas_id, None, true)?;
        self.save_manifest();
        Ok(rel)
    }

    /// Canvas file URLs redirect to a CDN that rejects our cookie, so the
    /// signed `public_url` is the one that actually downloads. `info.url` is
    /// the fallback for files that have none — but it comes back as `""` for
    /// anything we may not read, and an empty URL resolves to the Canvas home
    /// page, which would be saved as if it were the file.
    fn download_url(&self, info: &serde_json::Value, file_id: i64) -> Result<Option<String>, String> {
        if let Ok(r) = self.canvas.get(&format!("/api/v1/files/{file_id}/public_url")) {
            if r.ok() {
                if let Ok(j) = r.json() {
                    if let Some(u) = j["public_url"].as_str().filter(|u| !u.is_empty()) {
                        return Ok(Some(u.to_string()));
                    }
                }
            }
        }
        Ok(info["url"].as_str().filter(|u| !u.is_empty()).map(str::to_string))
    }

    fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let r = self.canvas.get(url)?;
        if !r.ok() {
            return Err(format!("download HTTP {}", r.status));
        }
        // A login page where a file should be means the session lapsed
        // mid-run; saving it would quietly corrupt the library.
        if r.content_type.contains("text/html") {
            return Err("got HTML instead of the file — session or URL problem".to_string());
        }
        Ok(r.body)
    }

    // ── Inline images ────────────────────────────────────────────────────────

    /// Convert a body to Markdown, downloading its inline images first and
    /// pointing the Markdown at the local copies.
    ///
    /// Images are shared per course, so they all live in one `images/` folder
    /// at the course root. `out_path` is where the Markdown itself will land:
    /// the app resolves image `src` against the *document's* directory, so a
    /// page one level down has to reach back up or the image 404s.
    fn convert(&self, html: &str, c: &Subject, out_path: &str) -> String {
        let up = up_to_course_root(out_path);
        let mut images = ImageMap::new();
        for (endpoint, src) in md::image_refs(html) {
            if src.is_empty() || images.contains_key(&src) {
                continue;
            }
            match self.fetch_image(c, &endpoint) {
                Ok(Some(path)) => {
                    images.insert(src, format!("{up}{path}"));
                }
                Ok(None) => {}
                Err(e) => self.reporter.log("warning", &c.code, &format!("image {endpoint}: {e}")),
            }
        }
        md::to_markdown(html, &images)
    }

    /// Returns the image's course-relative path (`images/…`), not yet adjusted
    /// for the referring document's depth.
    fn fetch_image(&self, c: &Subject, endpoint: &str) -> Result<Option<String>, String> {
        let r = self.canvas.get(endpoint)?;
        if !r.ok() {
            return Ok(None);
        }
        let info = r.json()?;

        let ct = content_type_of(&info);
        let ext = IMAGE_EXT
            .iter()
            .find(|(k, _)| *k == ct)
            .map(|(_, v)| *v)
            .unwrap_or("png");
        // Fall back to the id in the endpoint: without it, several images in
        // one course would all be written as `images/0.png`.
        let fid = info["id"].as_i64().unwrap_or_else(|| {
            endpoint
                .rsplit('/')
                .find_map(|seg| seg.parse::<i64>().ok())
                .unwrap_or(0)
        });
        let path = format!("images/{fid}.{ext}");

        // Same unchanged-skip as fetch_file: metadata match + on disk.
        let modified = info["modified_at"]
            .as_str()
            .or_else(|| info["updated_at"].as_str())
            .unwrap_or("")
            .to_string();
        let meta_size = info["size"].as_u64().unwrap_or(0);
        if !modified.is_empty() {
            let known = self
                .manifest
                .borrow()
                .get(&fid.to_string())
                .is_some_and(|(m, s)| *m == modified && *s == meta_size);
            let on_disk = paths::course_rel_path(&c.code, &path)
                .map_or(false, |rel| self.data_dir.join(rel).is_file());
            if known && on_disk {
                return Ok(Some(path));
            }
        }

        let Some(url) = self.download_url(&info, fid)? else { return Ok(None) };
        let bytes = self.fetch_bytes(&url)?;
        self.write(c, &path, &bytes, Some(fid))?;
        if !modified.is_empty() {
            self.manifest
                .borrow_mut()
                .insert(fid.to_string(), (modified, meta_size));
        }
        Ok(Some(path))
    }

    // ── Output ───────────────────────────────────────────────────────────────

    /// Write one artifact and announce it. Returns the path relative to the
    /// data directory (`courses/CODE/...`), which is what the database stores.
    fn write(&self, c: &Subject, rel_path: &str, data: &[u8], canvas_id: Option<i64>) -> Result<String, String> {
        self.write_from(c, rel_path, data, canvas_id, None)
    }

    /// `write`, plus the Canvas URL the artifact came from (see
    /// [`FileEvent::source_url`]).
    fn write_from(
        &self,
        c: &Subject,
        rel_path: &str,
        data: &[u8],
        canvas_id: Option<i64>,
        source_url: Option<String>,
    ) -> Result<String, String> {
        let (rel, size, action) = paths::write_course_bytes(&self.data_dir, &c.code, rel_path, data)?;
        // Changed bytes invalidate the old parse and embeddings. Purge the
        // artifacts before the parse trigger below, or the sidecar's
        // existence checks would skip the re-parse and keep serving stale
        // markdown and vectors.
        if action == paths::WriteAction::Updated {
            paths::purge_parse_artifacts(&self.data_dir, &rel);
        }
        self.reporter.file(&FileEvent {
            subject_id: c.id,
            code: c.code.clone(),
            relative_path: rel.clone(),
            size_bytes: size,
            category: paths::category_from_path(rel_path).to_string(),
            canvas_id,
            source_url,
            action: action.as_str(),
        });

        if self.parse_pdfs && rel_path.ends_with(".pdf") {
            self.trigger_parse(&rel, c.id, &c.code);
        }
        Ok(rel)
    }

    /// Hand the PDF to the Python sidecar without waiting. The app wants each
    /// deck queued the moment it lands so the UI can show parse progress
    /// alongside the download.
    ///
    /// Queued, not spawned per file. Every new PDF used to get its own detached
    /// thread, so a first sync of a 105-deck library opened 105 threads and
    /// fired 105 simultaneous POSTs at the sidecar. FastAPI runs its sync
    /// endpoints on a 40-slot threadpool, so 40 fast parses ran at once — and a
    /// fast parse costs ~2 GB that the process never gives back
    /// (`sidecar/parse_worker.py` has the measurements). That was the OOM.
    /// The sidecar now bounds itself too; this keeps the caller's thread and
    /// socket count flat instead of scaling with the library.
    fn trigger_parse(&self, rel: &str, subject_id: i64, code: &str) {
        let mut slot = self.parse_queue.borrow_mut();
        let tx = slot.get_or_insert_with(spawn_parse_workers);
        // A closed channel means the workers are gone; the parse is not worth
        // failing a scrape over — `oculus index` re-runs it, idempotently.
        let _ = tx.send(ParseJob {
            data_dir: self.data_dir.clone(),
            rel: rel.to_string(),
            subject_id,
            code: code.to_string(),
            ipc_port: self.ipc_port,
        });
    }
}

/// Bounds the number of open parse requests. The sidecar's global heavy-work
/// slot serializes the actual parser memory; more senders here would only wait
/// on that slot while holding a thread and socket open.
const PARSE_WORKERS: usize = 2;

struct ParseJob {
    data_dir: PathBuf,
    rel: String,
    subject_id: i64,
    code: String,
    ipc_port: u16,
}

/// Detached on purpose: `trigger_parse` has always been fire-and-forget, and a
/// scrape must finish reporting without waiting on the parse tier. When the
/// engine drops, the sender goes with it and the workers exit once the queue
/// they are holding has drained.
fn spawn_parse_workers() -> std::sync::mpsc::Sender<ParseJob> {
    let (tx, rx) = std::sync::mpsc::channel::<ParseJob>();
    let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
    for _ in 0..PARSE_WORKERS {
        let rx = std::sync::Arc::clone(&rx);
        std::thread::spawn(move || loop {
            // Bind the result so the guard drops before the parse runs —
            // holding it across `parse_pdf` would serialise the workers.
            let job = rx.lock().unwrap().recv();
            let Ok(job) = job else { return };
            match parse_pdf(&job.data_dir, &job.rel, job.subject_id, &job.code, job.ipc_port) {
                Ok(mode) => eprintln!("[oculus] parse-pdf {}: {mode}", job.rel),
                Err(e) => eprintln!("[oculus] parse-pdf {}: {e}", job.rel),
            }
        });
    }
    tx
}

/// A fast parse of a large deck is not instant, and `ureq` has no default
/// timeout — an unbounded wait here would hang a headless run forever.
const PARSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20 * 60);

/// Ask the sidecar to parse a PDF, blocking until markdown exists.
///
/// The sidecar returns once the *fast* pass is done and queues the slower
/// quality pass on its own thread, so this is seconds, not minutes. Idempotent:
/// an already-parsed PDF returns immediately.
///
/// `rel_path` is the *library file* (what the database and all events key on);
/// for Office documents the actual bytes parsed are its derived sibling PDF.
pub fn parse_pdf(
    data_dir: &Path,
    rel_path: &str,
    subject_id: i64,
    subject_code: &str,
    ipc_port: u16,
) -> Result<String, String> {
    let pdf_rel = paths::doc_pdf_rel(rel_path)
        .ok_or_else(|| format!("{rel_path}: no PDF representation to parse"))?;
    let body = serde_json::json!({
        "pdf_path": data_dir.join(&pdf_rel).to_string_lossy(),
        "subject_code": subject_code,
        "relative_path": rel_path,
        "subject_id": subject_id,
        "ipc_port": ipc_port,
        "mineru_token": crate::mineru::stored_api_key(),
    });
    let url = format!("http://127.0.0.1:{}/parse-pdf", crate::sidecar::SIDECAR_PORT);
    let text = ureq::post(&url)
        .timeout(PARSE_TIMEOUT)
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| format!("sidecar unavailable: {e}"))?
        .into_string()
        .map_err(|e| format!("unreadable sidecar response: {e}"))?;

    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    Ok(v["mode"].as_str().unwrap_or("ok").to_string())
}

// ── Office → PDF conversion ──────────────────────────────────────────────────

fn office_ext(ct: &str) -> Option<&'static str> {
    OFFICE_TYPES.iter().find(|(k, _)| *k == ct).map(|(_, v)| *v)
}

/// pptx/docx/ppt/doc → PDF via LibreOffice headless. Everything happens in a
/// scratch directory soffice writes into alone, so the read-back name is
/// unambiguous; the directory is removed whatever the outcome.
fn office_to_pdf(bytes: &[u8], ext: &str) -> Result<Vec<u8>, String> {
    let soffice = find_soffice().ok_or_else(|| {
        "LibreOffice not installed — `brew install --cask libreoffice` enables Office → PDF conversion"
            .to_string()
    })?;

    let scratch = std::env::temp_dir().join(format!(
        "oculus-office-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).map_err(|e| format!("scratch dir: {e}"))?;
    let result = convert_in(&soffice, &scratch, bytes, ext);
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

fn convert_in(soffice: &Path, dir: &Path, bytes: &[u8], ext: &str) -> Result<Vec<u8>, String> {
    let input = dir.join(format!("input.{ext}"));
    std::fs::write(&input, bytes).map_err(|e| format!("write temp: {e}"))?;

    // A private UserInstallation lets this run while the LibreOffice GUI is
    // open — soffice otherwise refuses to start a second instance.
    let profile = url::Url::from_file_path(dir.join("profile"))
        .map_err(|_| "profile path not absolute".to_string())?;
    let mut child = std::process::Command::new(soffice)
        .arg(format!("-env:UserInstallation={profile}"))
        .args(["--headless", "--norestore", "--convert-to", "pdf", "--outdir"])
        .arg(dir)
        .arg(&input)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("launch soffice: {e}"))?;

    // std has no wait-with-timeout, so poll.
    let deadline = std::time::Instant::now() + OFFICE_CONVERT_TIMEOUT;
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(s) => break s,
            None if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("conversion timed out".to_string());
            }
            None => std::thread::sleep(std::time::Duration::from_millis(200)),
        }
    };
    if !status.success() {
        return Err(format!("soffice exited with {status}"));
    }
    std::fs::read(dir.join("input.pdf")).map_err(|e| format!("no PDF produced: {e}"))
}

fn find_soffice() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("OCULUS_SOFFICE") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    [
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
        "/opt/homebrew/bin/soffice",
        "/usr/local/bin/soffice",
        "/usr/bin/soffice",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn items_of(module: &serde_json::Value) -> &[serde_json::Value] {
    module["items"].as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn content_type_of(info: &serde_json::Value) -> String {
    info["content-type"]
        .as_str()
        .or_else(|| info["content_type"].as_str())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

/// `courses/CODE/files/x.pdf` → `files/x.pdf`, for links written into module
/// TOCs (which live one directory down).
fn rel_within_course(rel: &str) -> String {
    rel.splitn(3, '/').nth(2).unwrap_or(rel).to_string()
}

/// The `../` prefix a document at `out_path` needs to reach the course root,
/// where shared assets like `images/` live.
fn up_to_course_root(out_path: &str) -> String {
    "../".repeat(out_path.matches('/').count())
}

/// `assignments/<slug>.md`, falling back to `<slug>-<id>.md` when two titles
/// slug identically — without this the second document overwrites the first.
fn task_path(dir: &str, title: &str, id: i64, used: &mut HashSet<String>) -> String {
    let base = format!("{dir}/{}", slug(title));
    if used.insert(base.clone()) {
        format!("{base}.md")
    } else {
        format!("{base}-{id}.md")
    }
}

/// Append `**Label:** <timestamp>` when Canvas supplied one.
/// `"submitted"`/`"graded"` when the current user has handed the task in,
/// `None` otherwise. From `include[]=submission` on the assignments API.
fn submission_status(item: &serde_json::Value) -> Option<&'static str> {
    match item["submission"]["workflow_state"].as_str() {
        Some("graded") => Some("graded"),
        Some("submitted") | Some("pending_review") => Some("submitted"),
        _ => None,
    }
}

fn push_ts(meta: &mut Vec<String>, label: &str, iso: Option<&str>) {
    if let Some(ts) = iso.filter(|s| !s.is_empty()) {
        meta.push(format!("**{label}:** {}", fmt_ts(ts)));
    }
}

/// `"2026-09-12T13:59:59Z"` → `"2026-09-12 13:59 UTC"`. Canvas timestamps are
/// UTC; without a timezone library the honest move is to keep them that way
/// and let the frontend localise.
fn fmt_ts(iso: &str) -> String {
    if iso.len() >= 16 && iso.as_bytes()[10] == b'T' {
        format!("{} {} UTC", &iso[..10], &iso[11..16])
    } else {
        iso.to_string()
    }
}

/// `20.0` → `"20"`, `12.5` → `"12.5"` — Canvas points are floats, titles not.
fn fmt_points(p: f64) -> String {
    if p.fract() == 0.0 {
        format!("{}", p as i64)
    } else {
        format!("{p}")
    }
}

fn escape_md(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            let esc = matches!(c, '*' | '_' | '`' | '[' | ']' | '\\');
            esc.then_some('\\').into_iter().chain(std::iter::once(c))
        })
        .collect()
}

/// Filename-safe, URL-ish slug. Capped so a long Canvas title cannot produce a
/// path the filesystem rejects.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c);
        } else {
            pending_dash = true;
        }
    }
    out.truncate(60);
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_lowercase_dashed_and_capped() {
        assert_eq!(slug("Welcome & Executive Summary"), "welcome-executive-summary");
        assert_eq!(slug("  --Trim-- "), "trim");
        assert_eq!(slug(""), "untitled");
        assert_eq!(slug("!!!"), "untitled");
        assert_eq!(slug(&"a".repeat(80)).len(), 60);
    }

    #[test]
    fn module_links_are_relative_to_the_course_root() {
        assert_eq!(rel_within_course("courses/ABC_2026/files/x.pdf"), "files/x.pdf");
        assert_eq!(rel_within_course("files/x.pdf"), "files/x.pdf");
    }

    #[test]
    fn assets_are_addressed_from_the_document_that_references_them() {
        // The viewer resolves an image src against the markdown file's own
        // directory, so a page has to climb back to the course root.
        assert_eq!(up_to_course_root("home.md"), "");
        assert_eq!(up_to_course_root("pages/week-one.md"), "../");
        assert_eq!(up_to_course_root("announcements/2026-08-14-x.md"), "../");
    }

    #[test]
    fn content_type_ignores_charset_and_either_spelling() {
        let a = serde_json::json!({ "content-type": "application/pdf; charset=utf-8" });
        let b = serde_json::json!({ "content_type": "image/png" });
        assert_eq!(content_type_of(&a), "application/pdf");
        assert_eq!(content_type_of(&b), "image/png");
        assert_eq!(content_type_of(&serde_json::json!({})), "");
    }
}

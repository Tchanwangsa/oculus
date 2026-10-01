//! The Canvas scrape engine. Modules drive the walk, and pages and files are
//! fetched through them, so nothing is downloaded twice. Lives in Rust, not a
//! WebView — see `docs/architecture.md`.
//!
//! Progress leaves through [`Reporter`]: the app forwards it as Tauri events,
//! the CLI prints it.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::agents;
use crate::canvas::Canvas;
use crate::md::{self, ImageMap};
use crate::parse;
use crate::paths;

/// Types stored as-is; everything downstream is PDF-shaped.
const DOWNLOADABLE_TYPES: &[&str] = &["application/pdf"];

/// Office formats kept as-is plus a LibreOffice-converted sibling PDF, mapped
/// to the extension the converter needs on its input file.
const OFFICE_TYPES: &[(&str, &str)] = &[
    ("application/vnd.openxmlformats-officedocument.presentationml.presentation", "pptx"),
    ("application/vnd.openxmlformats-officedocument.wordprocessingml.document", "docx"),
    ("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "xlsx"),
    ("application/vnd.ms-powerpoint", "ppt"),
    ("application/msword", "doc"),
    ("application/vnd.ms-excel", "xls"),
];

/// A wedged soffice must not hang the whole sync run.
const OFFICE_CONVERT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

/// Larger files are skipped rather than filling the disk with recordings.
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
    /// Set for pages: the URL slug survives renames while the filename tracks
    /// the title, so matching a body link to the local copy needs this.
    pub source_url: Option<String>,
    /// `"new"`, `"updated"`, or `"unchanged"` — feeds the per-run sync history.
    pub action: &'static str,
}

/// Announced before a download, under the same `relative_path` the eventual
/// [`FileEvent`] carries, so the UI can show "downloading" for a new file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileStart {
    pub subject_id: i64,
    pub code: String,
    pub relative_path: String,
    pub filename: String,
    pub size_bytes: u64,
}

/// Where a run's side effects go. Default methods are no-ops.
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

/// Discards everything.
pub struct Silent;
impl Reporter for Silent {}

// ── Engine ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Subject {
    pub id: i64,
    pub code: String,
}

/// Canvas id → course-relative path of each assignment/quiz document, keyed
/// the way module items refer to them (`content_id`).
#[derive(Debug, Default)]
pub struct TaskDocs {
    assignments: HashMap<i64, String>,
    quizzes: HashMap<i64, String>,
}

/// The per-course link crawl: every converted body queues the pages and files
/// it references, and `crawl_links` drains them depth-first after the content
/// phases, so anything reachable from any scraped body lands on disk.
#[derive(Debug, Default)]
struct LinkCrawl {
    seen_pages: HashSet<String>,
    seen_files: HashSet<String>,
    /// Pending page slugs / file ids, popped LIFO.
    pages: Vec<String>,
    files: Vec<String>,
}

impl LinkCrawl {
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
    /// The shape the frontend's `upsertSubjects` reads.
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

/// Which content categories a sync fetches; all on by default (the CLI
/// always syncs everything).
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
    pub ed: crate::ed::Ed,
    data_dir: PathBuf,
    reporter: Box<dyn Reporter>,
    parse_pdfs: bool,
    options: SyncOptions,
    /// Canvas file id → (modified_at, size) at last download
    /// (`file-manifest.json`). A matching pair with the artifact on disk skips
    /// the download.
    manifest: std::cell::RefCell<HashMap<String, (String, u64)>>,
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

    // ── Course list ──────────────────────────────────────────────────────────

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
            .max_by_key(|t| crate::terms::term_key(t))
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

    /// A subject that fails is logged and skipped. Returns how many subjects
    /// were attempted.
    pub fn scrape(&self, subjects: &[Subject]) -> usize {
        let total = subjects.len();

        // Before the per-course links below, so they never dangle.
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
            // Idempotent; gives a newly scraped subject its agent scaffold.
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

        // Separate from the front page — a course can have both.
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

    /// Writes `assignments/*.md` and `quizzes/*.md`; returns where each landed
    /// so the modules phase can link locally.
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

        // Quizzes get submission state through their assignment shell,
        // resolved up front since quiz documents are written first.
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

        // Classic quizzes first; their assignment shells are skipped below.
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

    /// Mirror the subject's Ed board into `ed/*.md`, one thread per file. A
    /// course with no Ed tool is skipped (session minting: `Ed::resolve_course`).
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

            // Ed's per-course thread number is stable across syncs.
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

        // Nested fetches are not counted, so the total holds still.
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
                        // TOCs live in modules/, so links step up a level.
                        toc.push(match saved {
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

    /// Drain the link crawl depth-first; the `seen_*` sets stop repeats and
    /// cycles. No progress — these are not module items.
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
        // The *requested* slug, not canonical `full["url"]`: old body links to
        // a renamed page still use it, and Canvas still resolves it.
        let source = format!(
            "{}/courses/{}/pages/{page_url}",
            crate::canvas::CANVAS_BASE,
            c.id
        );
        self.write_from(c, &out, md_body.as_bytes(), None, Some(source))?;
        Ok(Some(links))
    }

    /// Download one Canvas file if its type is allowlisted.
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

        let name = info["filename"]
            .as_str()
            .or_else(|| info["display_name"].as_str())
            .or(display)
            .unwrap_or("file.bin")
            .replace(['/', '\\'], "_");

        // Canvas serves some uploads as a generic binary (whatever the
        // uploader's browser claimed); those fall back to the extension.
        let ct = content_type_of(&info);
        let office = office_ext(&ct).or_else(|| is_generic_binary(&ct).then(|| office_ext_of(&name)).flatten());
        let downloadable = DOWNLOADABLE_TYPES.contains(&ct.as_str())
            || (is_generic_binary(&ct) && name.to_ascii_lowercase().ends_with(".pdf"));
        if !downloadable && office.is_none() {
            return Ok(None);
        }
        if info["size"].as_u64().unwrap_or(0) > MAX_FILE_BYTES {
            self.reporter.log("warning", &c.code, &format!("file {file_id}: over size cap, skipped"));
            return Ok(None);
        }
        // Canvas lists a release-dated file and answers its metadata but
        // refuses the download; check explicitly, or it reads as an auth failure.
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

        // Unchanged since last time and on disk (derived PDF too) → skip.
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

        // The original is the library file. Office documents get a derived
        // "deck.pptx.pdf" beside them — never announced, never a database row.
        let rel = self.write(c, &format!("files/{name}"), &bytes, Some(file_id))?;

        // A failed Office conversion stays out of the manifest so it retries.
        let mut complete = true;
        if let Some(ext) = office {
            match office_to_pdf(&bytes, ext) {
                Ok(pdf) => {
                    paths::write_course_bytes(&self.data_dir, &c.code, &format!("files/{name}.pdf"), &pdf)?;
                    if self.parse_pdfs {
                        self.trigger_parse(&rel, c.id);
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

    /// Re-download one file, bypassing the unchanged-skip.
    pub fn refetch_file(&self, c: &Subject, canvas_id: i64) -> Result<Option<String>, String> {
        let rel = self.fetch_file(c, canvas_id, None, true)?;
        self.save_manifest();
        Ok(rel)
    }

    /// Canvas file URLs redirect to a CDN that rejects our cookie, so prefer
    /// the signed `public_url`. An empty `info.url` must be refused: it would
    /// resolve to the Canvas home page and be saved as the file.
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

    /// Convert a body to Markdown with its inline images downloaded to the
    /// course's `images/`. Image `src` resolves against the document at
    /// `out_path`, so nested documents climb back to the course root.
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

    /// Returns `images/…`, not yet adjusted for the document's depth.
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
        // Without an id every image would be `images/0.png`.
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

    /// Write one artifact and announce it. Returns `courses/CODE/...`.
    fn write(&self, c: &Subject, rel_path: &str, data: &[u8], canvas_id: Option<i64>) -> Result<String, String> {
        self.write_from(c, rel_path, data, canvas_id, None)
    }

    fn write_from(
        &self,
        c: &Subject,
        rel_path: &str,
        data: &[u8],
        canvas_id: Option<i64>,
        source_url: Option<String>,
    ) -> Result<String, String> {
        let (rel, size, action) = paths::write_course_bytes(&self.data_dir, &c.code, rel_path, data)?;
        // Purge the stale parse before the trigger below, or its skip check
        // would keep serving the old markdown and vectors.
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
            self.trigger_parse(&rel, c.id);
        }
        Ok(rel)
    }

    /// Start the parse without waiting for it. One detached thread per PDF,
    /// deliberately unpooled: concurrency belongs to the batcher
    /// (`parse/mineru/batch.rs`), and a gate here would split its batches.
    fn trigger_parse(&self, rel: &str, subject_id: i64) {
        let data_dir = self.data_dir.clone();
        let rel = rel.to_string();
        // Not worth failing a scrape over — `oculus index` re-runs the parse.
        let _ = std::thread::Builder::new().name("oculus-parse".into()).spawn(move || {
            match parse_pdf(&data_dir, &rel, subject_id) {
                Ok(summary) => eprintln!("[oculus] parse-pdf {rel}: {summary}"),
                Err(e) => eprintln!("[oculus] parse-pdf {rel}: {e}"),
            }
        });
    }
}

/// What a finished `parse_pdf` did.
#[derive(Debug, Clone)]
pub struct ParseSummary {
    /// The PDF already had a current `.pages.json`; nothing was sent.
    pub skipped: bool,
    pub pages: u32,
    pub images: u32,
    /// Page rows written to `pages`. Zero with `skipped` false means no
    /// database or no file row yet; the artifacts are still on disk.
    pub pages_recorded: usize,
}

impl std::fmt::Display for ParseSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.skipped {
            write!(f, "already parsed ({} pages", self.pages)?;
            if self.pages_recorded > 0 {
                write!(f, ", {} records folded in", self.pages_recorded)?;
            }
            return write!(f, ")");
        }
        write!(f, "{} pages, {} images, {} recorded", self.pages, self.images, self.pages_recorded)
    }
}

/// Parse a PDF, blocking (for minutes) until the artifacts are on disk.
/// Deliberately no timeout here — the client's `POLL_DEADLINE` is the only
/// one (see `docs/parsing.md`). Idempotent.
///
/// `rel_path` is the library file; for Office documents the bytes parsed are
/// its derived sibling PDF.
pub fn parse_pdf(
    data_dir: &Path,
    rel_path: &str,
    subject_id: i64,
) -> Result<ParseSummary, parse::ParseError> {
    parse_pdf_reporting(data_dir, rel_path, subject_id, &|_| {})
}

/// `parse_pdf` plus a progress callback, for the CLI (the app reads the
/// `parse-status` events).
pub fn parse_pdf_reporting(
    data_dir: &Path,
    rel_path: &str,
    subject_id: i64,
    on_progress: &dyn Fn(parse::Progress),
) -> Result<ParseSummary, parse::ParseError> {
    match run_parse(data_dir, rel_path, subject_id, on_progress) {
        Ok(summary) => Ok(summary),
        Err(error) => {
            parse::events::failed(rel_path, subject_id, &error);
            Err(error)
        }
    }
}

fn run_parse(
    data_dir: &Path,
    rel_path: &str,
    subject_id: i64,
    on_progress: &dyn Fn(parse::Progress),
) -> Result<ParseSummary, parse::ParseError> {
    // Local failures: `Io` is retryable, not latching, so a missing file never
    // stops the rest of the library parsing.
    let pdf_rel = paths::doc_pdf_rel(rel_path).ok_or_else(|| {
        parse::ParseError::Io(format!("{rel_path} has no PDF representation to parse"))
    })?;
    let pdf = data_dir.join(&pdf_rel);
    if !pdf.is_file() {
        return Err(parse::ParseError::Io(format!("not on disk: {}", pdf.display())));
    }

    if parse::parse_mode(&pdf).is_some() {
        // An artifact on disk is no promise its page rows exist; backfill.
        let record = parse::read_record(&pdf);
        let pages = record.as_ref().map(|r| r.page_count).unwrap_or(0);
        let pages_recorded = match record {
            Some(record) => backfill_pages(data_dir, rel_path, subject_id, &record)
                .unwrap_or_else(|e| {
                    eprintln!("[oculus] parse-pdf {rel_path}: page records not backfilled: {e}");
                    0
                }),
            None => 0,
        };
        // A sweep that kicked this row is waiting for a terminal status.
        parse::events::parsed(rel_path, subject_id);
        return Ok(ParseSummary { skipped: true, pages, images: 0, pages_recorded });
    }

    parse::events::queued(rel_path, subject_id);

    let parser = parse::backend()?;
    parse::preflight(parser.as_ref())?;

    let staging = parse::ImageStaging::begin(&pdf)?;
    let output = parser.parse(&pdf, staging.dir(), staging.rel(), &|progress| {
        parse::events::running(rel_path, subject_id, progress);
        on_progress(progress);
    })?;
    // `.pages.json` is the only evidence a parse finished; written last, atomically.
    output.write(&pdf, staging)?;

    // Best-effort: a database problem must not fail a successful parse;
    // `oculus index` folds it in later.
    let pages_recorded = match record_pages(data_dir, rel_path, subject_id, &output) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("[oculus] parse-pdf {rel_path}: page records not written: {e}");
            0
        }
    };

    parse::events::parsed(rel_path, subject_id);
    Ok(ParseSummary {
        skipped: false,
        pages: output.page_count,
        images: output.image_count,
        pages_recorded,
    })
}

/// Fold an already-parsed file's record in only if it has no rows yet —
/// unlike `record_pages`, which always writes fresh text.
fn backfill_pages(
    data_dir: &Path,
    rel_path: &str,
    subject_id: i64,
    record: &parse::ParseOutput,
) -> Result<usize, String> {
    tauri::async_runtime::block_on(async move {
        let pool = crate::store::open(data_dir).await?;
        let result = async {
            let Some(file_id) = crate::store::file_id(&pool, subject_id, rel_path).await? else {
                return Ok(0);
            };
            if crate::store::page_count(&pool, file_id).await? > 0 {
                return Ok(0);
            }
            crate::store::upsert_pages(&pool, file_id, &record.pages).await
        }
        .await;
        pool.close().await;
        result
    })
}

/// Fold a finished parse into the `pages` table.
fn record_pages(
    data_dir: &Path,
    rel_path: &str,
    subject_id: i64,
    output: &parse::ParseOutput,
) -> Result<usize, String> {
    tauri::async_runtime::block_on(async move {
        let pool = crate::store::open(data_dir).await?;
        let result = match crate::store::file_id(&pool, subject_id, rel_path).await? {
            Some(file_id) => crate::store::upsert_pages(&pool, file_id, &output.pages).await,
            // A parse can outrun the row: the frontend writes `files` from
            // scrape events, and a CLI run may have no database write at all.
            None => Ok(0),
        };
        pool.close().await;
        result
    })
}

// ── Office → PDF conversion ──────────────────────────────────────────────────

fn office_ext(ct: &str) -> Option<&'static str> {
    OFFICE_TYPES.iter().find(|(k, _)| *k == ct).map(|(_, v)| *v)
}

/// The only case where the filename decides the type.
fn is_generic_binary(ct: &str) -> bool {
    matches!(ct, "" | "application/octet-stream" | "binary/octet-stream")
}

/// The converter extension an untyped file's name claims, if known.
pub(crate) fn office_ext_of(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    OFFICE_TYPES
        .iter()
        .map(|(_, e)| *e)
        .find(|e| lower.ends_with(&format!(".{e}")))
}

/// Office → PDF via headless LibreOffice, in a private scratch directory that
/// is removed whatever the outcome.
pub(crate) fn office_to_pdf(bytes: &[u8], ext: &str) -> Result<Vec<u8>, String> {
    let soffice = find_soffice().ok_or_else(|| {
        "LibreOffice not installed — `brew install --cask libreoffice` enables Office → PDF conversion"
            .to_string()
    })?;

    let scratch = std::env::temp_dir().join(format!(
        "oculus-office-{}-{}",
        std::process::id(),
        crate::clock::now_nanos()
    ));
    std::fs::create_dir_all(&scratch).map_err(|e| format!("scratch dir: {e}"))?;
    let result = convert_in(&soffice, &scratch, bytes, ext);
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

/// Calc slices a wide sheet into header-less page-width columns, so
/// spreadsheets export with `SinglePageSheets`; the resulting huge page is
/// kept in bounds by `embed/raster.rs::dpi_for_page`.
fn convert_target(ext: &str) -> &'static str {
    match ext {
        "xlsx" | "xls" => {
            r#"pdf:calc_pdf_Export:{"SinglePageSheets":{"type":"boolean","value":"true"}}"#
        }
        _ => "pdf",
    }
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
        .args(["--headless", "--norestore", "--convert-to"])
        .arg(convert_target(ext))
        .arg("--outdir")
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

/// `courses/CODE/files/x.pdf` → `files/x.pdf`.
fn rel_within_course(rel: &str) -> String {
    rel.splitn(3, '/').nth(2).unwrap_or(rel).to_string()
}

/// The `../` prefix a document at `out_path` needs to reach the course root.
fn up_to_course_root(out_path: &str) -> String {
    "../".repeat(out_path.matches('/').count())
}

/// `<dir>/<slug>.md`, or `<slug>-<id>.md` when two titles slug identically.
fn task_path(dir: &str, title: &str, id: i64, used: &mut HashSet<String>) -> String {
    let base = format!("{dir}/{}", slug(title));
    if used.insert(base.clone()) {
        format!("{base}.md")
    } else {
        format!("{base}-{id}.md")
    }
}

/// `"submitted"`/`"graded"` when the user has handed the task in. From
/// `include[]=submission` on the assignments API.
fn submission_status(item: &serde_json::Value) -> Option<&'static str> {
    match item["submission"]["workflow_state"].as_str() {
        Some("graded") => Some("graded"),
        Some("submitted") | Some("pending_review") => Some("submitted"),
        _ => None,
    }
}

/// Append `**Label:** <timestamp>` when Canvas supplied one.
fn push_ts(meta: &mut Vec<String>, label: &str, iso: Option<&str>) {
    if let Some(ts) = iso.filter(|s| !s.is_empty()) {
        meta.push(format!("**{label}:** {}", fmt_ts(ts)));
    }
}

/// `"2026-09-12T13:59:59Z"` → `"2026-09-12 13:59 UTC"`; left in UTC.
fn fmt_ts(iso: &str) -> String {
    if iso.len() >= 16 && iso.as_bytes()[10] == b'T' {
        format!("{} {} UTC", &iso[..10], &iso[11..16])
    } else {
        iso.to_string()
    }
}

/// `20.0` → `"20"`, `12.5` → `"12.5"`.
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

/// Filename-safe slug, capped so the filesystem never rejects the path.
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
        assert_eq!(up_to_course_root("home.md"), "");
        assert_eq!(up_to_course_root("pages/week-one.md"), "../");
        assert_eq!(up_to_course_root("announcements/2026-08-14-x.md"), "../");
    }

    #[test]
    fn spreadsheets_convert_like_every_other_office_format() {
        assert_eq!(
            office_ext("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
            Some("xlsx")
        );
        assert_eq!(office_ext("application/vnd.ms-excel"), Some("xls"));
        assert_eq!(office_ext("application/zip"), None);
    }

    #[test]
    fn only_spreadsheets_ask_calc_to_stop_slicing_the_sheet() {
        assert_eq!(convert_target("pptx"), "pdf");
        assert_eq!(convert_target("docx"), "pdf");
        assert!(convert_target("xlsx").contains("SinglePageSheets"));
        assert!(convert_target("xls").starts_with("pdf:calc_pdf_Export:"));
    }

    #[test]
    fn an_untyped_upload_falls_back_to_its_extension() {
        // The longer extension has to win, or "deck.pptx" converts as "ppt".
        assert_eq!(office_ext_of("deck.pptx"), Some("pptx"));
        assert_eq!(office_ext_of("old deck.PPT"), Some("ppt"));
        assert_eq!(office_ext_of("marks.xlsx"), Some("xlsx"));
        // Not an Office format, so the name buys it nothing.
        assert_eq!(office_ext_of("archive.zip"), None);
        assert_eq!(office_ext_of("notes.pdf"), None);

        assert!(is_generic_binary(""));
        assert!(is_generic_binary("application/octet-stream"));
        assert!(!is_generic_binary("application/pdf"));
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

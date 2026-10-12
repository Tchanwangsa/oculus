//! Phase: the subject's Ed course — its Discussion board and its Lessons.

use crate::sources::ed::{self, Board};
use crate::sync::render::slug;
use crate::sync::{Engine, Progress, Subject};

impl Engine {
    /// Resolve the subject's Ed course once (session minting:
    /// `Ed::resolve_course`), then mirror whichever of threads and lessons the
    /// options ask for. A course with no Ed tool is skipped; a failure in one
    /// half is a warning and never costs the other.
    pub(in crate::sync) fn scrape_ed(&self, c: &Subject) {
        let course_id = match self.ed.resolve_course(&self.canvas, c.id, &c.code) {
            Ok(Some(id)) => id,
            Ok(None) => return,
            Err(e) => {
                self.reporter.log("info", &c.code, &format!("ed: {e}"));
                return;
            }
        };
        // Lessons first: a few dozen requests, where the board re-reads every
        // thread and is the half that meets Ed's rate limit.
        if self.options.ed_lessons {
            if let Err(e) = self.scrape_ed_lessons(c, course_id) {
                self.reporter
                    .log("warning", &c.code, &format!("ed lessons: {e}"));
            }
        }
        if self.options.ed && !self.reporter.cancelled() {
            if let Err(e) = self.scrape_ed_threads(c, course_id) {
                self.reporter.log("warning", &c.code, &format!("ed: {e}"));
            }
        }
    }

    /// The board into `ed/*.md`, one thread per file, each with its page in
    /// Ed as the file's source.
    fn scrape_ed_threads(&self, c: &Subject, course_id: i64) -> Result<(), String> {
        let threads = self.ed.threads(course_id)?;
        let region = self.ed.region();
        let categories = self.ed.discussion_categories(course_id);
        let board = Board {
            region: region.as_deref(),
            course_id,
            categories: &categories,
        };

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
            let url = board
                .region
                .zip(t["id"].as_i64())
                .map(|(region, id)| ed::thread_url(region, course_id, id));
            match self.ed.thread_markdown(t, &board) {
                Ok(md) => {
                    self.write_from(c, &path, md.as_bytes(), None, url)?;
                }
                Err(e) => self
                    .reporter
                    .log("warning", &c.code, &format!("ed thread {title}: {e}")),
            }
        }
        Ok(())
    }
}

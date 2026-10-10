//! Phase: the subject's Ed Discussion board.

use crate::sync::render::slug;
use crate::sync::{Engine, Progress, Subject};

impl Engine {
    /// Mirror the subject's Ed board into `ed/*.md`, one thread per file. A
    /// course with no Ed tool is skipped (session minting: `Ed::resolve_course`).
    pub(in crate::sync) fn scrape_ed(&self, c: &Subject) -> Result<(), String> {
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
                Err(e) => self
                    .reporter
                    .log("warning", &c.code, &format!("ed thread {title}: {e}")),
            }
        }
        Ok(())
    }
}

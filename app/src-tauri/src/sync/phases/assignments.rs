//! Phase: assignments and quizzes, with the documents the modules phase links to.

use std::collections::HashSet;

use crate::pages::md;
use crate::sync::render::{fmt_points, push_ts, submission_status, task_path};
use crate::sync::{Engine, LinkCrawl, Progress, Subject, TaskDocs};

impl Engine {
    /// Writes `assignments/*.md` and `quizzes/*.md`; returns where each landed
    /// so the modules phase can link locally.
    pub(in crate::sync) fn scrape_assignments(
        &self,
        c: &Subject,
        crawl: &mut LinkCrawl,
    ) -> Result<TaskDocs, String> {
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
                    meta.push(format!(
                        "**Submission:** {}",
                        kinds.join(", ").replace('_', " ")
                    ));
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
}

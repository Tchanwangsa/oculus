//! Phase: modules, which drives pages and files, then the link crawl.

use crate::pages::md;
use crate::sync::render::{escape_md, file_toc_line, items_of, slug};
use crate::sync::{Engine, Fetched, LinkCrawl, Progress, Subject, TaskDocs};

impl Engine {
    pub(in crate::sync) fn scrape_modules(
        &self,
        c: &Subject,
        tasks: &TaskDocs,
        crawl: &mut LinkCrawl,
    ) -> Result<(), String> {
        let modules = self.canvas.get_all(&format!(
            "/api/v1/courses/{}/modules?include[]=items&per_page=100",
            c.id
        ))?;

        // Nested fetches are not counted, so the total holds still.
        let total_items: usize = modules
            .iter()
            .map(|m| {
                items_of(m)
                    .iter()
                    .filter(|it| it["type"] != "SubHeader")
                    .count()
            })
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
                        let Some(page_url) = item["page_url"].as_str() else {
                            continue;
                        };
                        if crawl.seen_pages.insert(page_url.to_string()) {
                            match self.fetch_page(c, page_url, title) {
                                Ok(Some(links)) => crawl.absorb(links),
                                Ok(None) => {}
                                Err(e) => self.reporter.log(
                                    "warning",
                                    &c.code,
                                    &format!("page {page_url}: {e}"),
                                ),
                            }
                        }
                        toc.push(format!(
                            "{indent}- [{}](../pages/{}.md)",
                            escape_md(title),
                            slug(title)
                        ));
                    }
                    "File" => {
                        let Some(id) = item["content_id"].as_i64() else {
                            continue;
                        };
                        let mut fetched = Fetched::Skipped;
                        if crawl.seen_files.insert(id.to_string()) {
                            match self.fetch_file(c, id, Some(title)) {
                                Ok(f) => fetched = f,
                                Err(e) => self.reporter.log(
                                    "warning",
                                    &c.code,
                                    &format!("file {id}: {e}"),
                                ),
                            }
                        }
                        toc.push(file_toc_line(&indent, title, &fetched));
                    }
                    "Assignment" | "Quiz" => {
                        let kind = if ty == "Quiz" { "quiz" } else { "assignment" };
                        let map = if ty == "Quiz" {
                            &tasks.quizzes
                        } else {
                            &tasks.assignments
                        };
                        let local = item["content_id"].as_i64().and_then(|id| map.get(&id));
                        toc.push(match local {
                            Some(path) => {
                                format!("{indent}- [{}](../{path}) _({kind})_", escape_md(title))
                            }
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
                        item["external_url"]
                            .as_str()
                            .or(item["html_url"].as_str())
                            .unwrap_or("")
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
    pub(in crate::sync) fn crawl_links(&self, c: &Subject, crawl: &mut LinkCrawl) {
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
                    Err(e) => {
                        self.reporter
                            .log("warning", &c.code, &format!("page {page_url}: {e}"))
                    }
                }
                continue;
            }
            let id = crawl
                .files
                .pop()
                .expect("loop guard: one stack is non-empty");
            if !crawl.seen_files.insert(id.clone()) {
                continue;
            }
            let Ok(fid) = id.parse::<i64>() else { continue };
            if let Err(e) = self.fetch_file(c, fid, None) {
                self.reporter
                    .log("warning", &c.code, &format!("file {id}: {e}"));
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
        let r = self
            .canvas
            .get(&format!("/api/v1/courses/{}/pages/{page_url}", c.id))?;
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
            if updated.is_empty() {
                String::new()
            } else {
                format!("_Updated: {updated}_\n\n")
            },
            self.convert(body, c, &out)
        );
        // The *requested* slug, not canonical `full["url"]`: old body links to
        // a renamed page still use it, and Canvas still resolves it.
        let source = format!(
            "{}/courses/{}/pages/{page_url}",
            crate::sources::canvas::CANVAS_BASE,
            c.id
        );
        self.write_from(c, &out, md_body.as_bytes(), None, Some(source))?;
        Ok(Some(links))
    }
}

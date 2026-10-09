//! Phase: announcements.

use crate::pages::md;
use crate::sync::render::slug;
use crate::sync::{Engine, LinkCrawl, Progress, Subject};

impl Engine {
    pub(in crate::sync) fn scrape_announcements(
        &self,
        c: &Subject,
        crawl: &mut LinkCrawl,
    ) -> Result<(), String> {
        let list = self.canvas.get_all(&format!(
            "{}/api/v1/courses/{}/discussion_topics?only_announcements=true&per_page=100&include[]=author",
            crate::sources::canvas::CANVAS_BASE,
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

            let Some(message) = a["message"].as_str().filter(|s| !s.is_empty()) else {
                continue;
            };
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
}

//! Phase: the course home page and syllabus.

use crate::pages::md;
use crate::sync::{Engine, LinkCrawl, Subject};

impl Engine {
    pub(in crate::sync) fn scrape_home(
        &self,
        c: &Subject,
        crawl: &mut LinkCrawl,
    ) -> Result<(), String> {
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
            .map(|a| {
                a.iter()
                    .filter_map(|t| t["display_name"].as_str())
                    .collect()
            })
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
            let md = format!(
                "{}\n---\n\n{body}",
                header(&format!("{name} — Syllabus"), "")
            );
            self.write(c, "syllabus.md", md.as_bytes(), None)?;
        }

        let (body, source) = match self
            .canvas
            .get(&format!("/api/v1/courses/{}/front_page", c.id))
        {
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
            match course["public_description"]
                .as_str()
                .filter(|s| !s.is_empty())
            {
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
}

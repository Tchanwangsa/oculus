//! Listing a board's threads and rendering one to Markdown.

use super::render::{author_name, content_md, fmt_ts, render_reply};
use super::{Ed, MAX_THREADS};

use std::collections::HashMap;

impl Ed {
    /// Every thread on a course's board, newest first (list entries only —
    /// no replies; those come with [`Ed::thread_markdown`]).
    pub fn threads(&self, course_id: i64) -> Result<Vec<serde_json::Value>, String> {
        let mut out = Vec::new();
        loop {
            let batch = self.get(&format!(
                "/courses/{course_id}/threads?limit=100&offset={}&sort=new",
                out.len()
            ))?;
            let Some(threads) = batch["threads"].as_array() else {
                break;
            };
            let n = threads.len();
            out.extend(threads.iter().cloned());
            if n < 100 || out.len() >= MAX_THREADS {
                break;
            }
        }
        Ok(out)
    }

    /// One thread with its replies, rendered to Markdown.
    pub fn thread_markdown(&self, listing: &serde_json::Value) -> Result<String, String> {
        let id = listing["id"].as_i64().ok_or("thread without id")?;
        let detail = self.get(&format!("/threads/{id}?view=1"))?;
        let thread = if detail["thread"].is_object() {
            &detail["thread"]
        } else {
            listing
        };

        // The roster may sit at either level; comments may embed their author.
        let mut users: HashMap<i64, String> = HashMap::new();
        for list in [&detail["users"], &detail["thread"]["users"]] {
            if let Some(arr) = list.as_array() {
                for u in arr {
                    if let (Some(id), Some(name)) = (u["id"].as_i64(), u["name"].as_str()) {
                        users.insert(id, name.to_string());
                    }
                }
            }
        }
        if let (Some(id), Some(name)) = (
            listing["user"]["id"].as_i64(),
            listing["user"]["name"].as_str(),
        ) {
            users.insert(id, name.to_string());
        }

        let title = thread["title"]
            .as_str()
            .or(listing["title"].as_str())
            .unwrap_or("Thread");
        let mut md = format!("# {title}\n\n");

        let mut meta = Vec::new();
        if let Some(n) = thread["number"].as_i64() {
            meta.push(format!("#{n}"));
        }
        let kind = thread["type"].as_str().unwrap_or("");
        if !kind.is_empty() {
            meta.push(kind.to_string());
        }
        // Surfaced so the board view can badge questions.
        if kind == "question" {
            let answered = [thread, listing].iter().any(|t| {
                t["is_answered"].as_bool().unwrap_or(false)
                    || t["is_staff_answered"].as_bool().unwrap_or(false)
            });
            meta.push(if answered { "resolved" } else { "unresolved" }.to_string());
        }
        let category = [
            thread["category"].as_str(),
            thread["subcategory"].as_str(),
            thread["subsubcategory"].as_str(),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
        if !category.is_empty() {
            meta.push(category);
        }
        md.push_str(&format!("**{}**  \n", meta.join(" · ")));
        md.push_str(&format!(
            "**By:** {} · {}\n\n---\n\n",
            author_name(thread, &users),
            fmt_ts(thread["created_at"].as_str().unwrap_or(""))
        ));

        md.push_str(&content_md(thread));
        md.push('\n');

        let mut replies = String::new();
        for a in thread["answers"].as_array().into_iter().flatten() {
            render_reply(a, &users, true, 0, &mut replies);
        }
        for c in thread["comments"].as_array().into_iter().flatten() {
            render_reply(c, &users, false, 0, &mut replies);
        }
        if !replies.is_empty() {
            md.push_str("\n## Replies\n");
            md.push_str(&replies);
        }

        Ok(crate::pages::md::collapse_blank_lines(&md)
            .trim()
            .to_string()
            + "\n")
    }
}

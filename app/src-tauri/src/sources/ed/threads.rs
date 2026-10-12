//! Listing a board's threads and rendering one to Markdown with front matter.

use super::front::{front_matter, text_or_null};
use super::palette::category_color;
use super::render::{author_name, content_md, fmt_ts, render_reply};
use super::{Ed, MAX_THREADS};

use serde_json::json;

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

    /// One thread with its replies, rendered to Markdown ([`thread_md`]).
    pub fn thread_markdown(
        &self,
        listing: &serde_json::Value,
        board: &Board,
    ) -> Result<String, String> {
        let id = listing["id"].as_i64().ok_or("thread without id")?;
        let detail = self.get(&format!("/threads/{id}"))?;
        Ok(thread_md(listing, &detail, board))
    }
}

/// What a thread's file needs beyond the thread itself.
pub struct Board<'a> {
    /// Ed's web region; no links while it is unknown.
    pub region: Option<&'a str>,
    pub course_id: i64,
    /// The course's top-level categories in Ed's order, for their colours.
    pub categories: &'a [String],
}

/// The web app's page for a thread in Ed's `region`.
pub fn thread_url(region: &str, course_id: i64, thread_id: i64) -> String {
    format!("https://edstem.org/{region}/courses/{course_id}/discussion/{thread_id}")
}

/// A thread's front matter, then its Markdown: title, a meta line, the post
/// and its replies. `detail` is `/threads/<id>`'s answer, `listing` the
/// board's entry, which stands in for a detail without a thread.
pub fn thread_md(listing: &serde_json::Value, detail: &serde_json::Value, board: &Board) -> String {
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
    let answered = (kind == "question").then(|| {
        [thread, listing].iter().any(|t| {
            t["is_answered"].as_bool().unwrap_or(false)
                || t["is_staff_answered"].as_bool().unwrap_or(false)
        })
    });
    if let Some(answered) = answered {
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

    let front = thread_front(thread, listing, answered, board);
    front + crate::pages::md::collapse_blank_lines(&md).trim() + "\n"
}

/// The front matter: the thread's ids, kind, answered state (only for a
/// question, as the meta line's resolved/unresolved), category path and
/// colour, and its page in Ed.
fn thread_front(
    thread: &serde_json::Value,
    listing: &serde_json::Value,
    answered: Option<bool>,
    board: &Board,
) -> String {
    let id = thread["id"].as_i64().or(listing["id"].as_i64());
    let category = thread["category"].as_str().filter(|s| !s.is_empty());
    let color = category.and_then(|c| category_color(board.categories, c));
    let url = board
        .region
        .zip(id)
        .map(|(region, id)| thread_url(region, board.course_id, id));
    front_matter(&[
        ("ed_thread_id", json!(id)),
        ("number", json!(thread["number"].as_i64())),
        ("type", text_or_null(&thread["type"])),
        ("answered", json!(answered)),
        ("category", json!(category)),
        ("subcategory", text_or_null(&thread["subcategory"])),
        ("subsubcategory", text_or_null(&thread["subsubcategory"])),
        ("category_color", json!(color)),
        ("url", json!(url)),
    ])
}

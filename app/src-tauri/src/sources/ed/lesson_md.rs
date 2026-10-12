//! One Ed lesson → one Markdown file, every slide in order. Pure: whatever the
//! sync fetched for a slide (quiz questions, the PDF and workspace files it
//! stored) arrives as [`SlideExtras`], keyed by slide id.
//!
//! The file sits at `lessons/<lessonId>-<slug>.md` and its assets below
//! `lessons/<lessonId>-<slug>/`, so every local link is relative to the
//! file's folder: `<lessonId>-<slug>/<NN>-<slide-slug>.pdf`.

use super::document::document_md;
use super::render::fmt_ts;

use std::collections::HashMap;

/// What a slide links to or shows beyond its own JSON.
#[derive(Default)]
pub struct SlideExtras {
    /// The downloaded PDF, course-relative.
    pub pdf: Option<String>,
    pub questions: Vec<serde_json::Value>,
    pub files: Vec<LinkedFile>,
    /// Says what could not be fetched for this slide.
    pub note: Option<String>,
}

/// A workspace file stored in the library.
pub struct LinkedFile {
    /// Its path inside the workspace, shown as the link text.
    pub path: String,
    /// Where it was stored, course-relative.
    pub rel: String,
    /// The text to show inline, for small text files.
    pub text: Option<String>,
}

/// The web app's page for a lesson in Ed's `region`.
pub fn lesson_url(region: &str, course_id: i64, lesson_id: i64) -> String {
    format!("https://edstem.org/{region}/courses/{course_id}/lessons/{lesson_id}")
}

/// A lesson's slides by `index`, then id.
pub fn sorted_slides(lesson: &serde_json::Value) -> Vec<&serde_json::Value> {
    let mut slides: Vec<&serde_json::Value> =
        lesson["slides"].as_array().into_iter().flatten().collect();
    slides.sort_by_key(|s| {
        (
            s["index"].as_i64().unwrap_or(i64::MAX),
            s["id"].as_i64().unwrap_or(i64::MAX),
        )
    });
    slides
}

/// The lesson as markdown; `url` is its page in Ed, linked when known.
pub fn lesson_md(
    lesson: &serde_json::Value,
    url: Option<&str>,
    module: Option<&str>,
    extras: &HashMap<i64, SlideExtras>,
) -> String {
    let title = lesson["title"].as_str().unwrap_or("Lesson");
    let mut md = format!("# {title}\n\n");

    if let Some(module) = module.filter(|m| !m.is_empty()) {
        md.push_str(&format!("**Module:** {module}  \n"));
    }
    let dates: Vec<String> = [
        ("Available", "available_at"),
        ("Due", "due_at"),
        ("Locks", "locked_at"),
    ]
    .into_iter()
    .filter_map(|(label, key)| {
        let at = lesson[key].as_str().filter(|s| !s.is_empty())?;
        Some(format!("**{label}:** {}", fmt_ts(at)))
    })
    .collect();
    if !dates.is_empty() {
        md.push_str(&format!("{}  \n", dates.join(" · ")));
    }
    if let Some(url) = url {
        md.push_str(&format!("[Open in Ed]({url})\n\n"));
    }
    md.push_str("---\n\n");

    let none = SlideExtras::default();
    for (i, slide) in sorted_slides(lesson).into_iter().enumerate() {
        let slide_title = slide["title"]
            .as_str()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("Slide {}", i + 1));
        let extra = slide["id"]
            .as_i64()
            .and_then(|id| extras.get(&id))
            .unwrap_or(&none);
        md.push_str(&format!("## {slide_title}\n\n"));
        md.push_str(&slide_body(slide, &slide_title, extra));
        md.push_str("\n\n");
    }

    crate::pages::md::collapse_blank_lines(&md)
        .trim()
        .to_string()
        + "\n"
}

fn slide_body(slide: &serde_json::Value, title: &str, extra: &SlideExtras) -> String {
    let text = |key: &str| slide[key].as_str().unwrap_or("");
    let mut parts: Vec<String> = Vec::new();
    match text("type") {
        "document" => parts.push(ed_md(text("content"))),
        "pdf" => match &extra.pdf {
            Some(rel) => parts.push(format!("[{} (PDF)]({})", link_text(title), local_link(rel))),
            None if !text("file_url").is_empty() => parts.push(format!(
                "[{} (PDF)]({})",
                link_text(title),
                text("file_url")
            )),
            None => {}
        },
        "code" | "jupyter" => {
            parts.push(ed_md(text("content")));
            parts.push(files_md(&extra.files));
        }
        "quiz" => {
            parts.push(ed_md(text("passage")));
            for (n, q) in extra.questions.iter().enumerate() {
                parts.push(question_md(n + 1, q));
            }
        }
        "html" => parts.push(demote_headings(&crate::pages::md::to_markdown(
            text("html"),
            &crate::pages::md::ImageMap::new(),
        ))),
        "video" => {
            if !text("video_url").is_empty() {
                parts.push(format!("[▶ {}]({})", link_text(title), text("video_url")));
            }
            parts.push(ed_md(text("content")));
        }
        "webpage" => {
            if !text("url").is_empty() {
                parts.push(format!("[{}]({})", link_text(title), text("url")));
            }
        }
        other => parts.push(format!("_Unsupported Ed slide type {other}._")),
    }
    if let Some(note) = &extra.note {
        parts.push(format!("_{note}_"));
    }
    parts
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Ed XML as Markdown sitting under a slide's heading.
fn ed_md(xml: &str) -> String {
    demote_headings(&document_md(xml))
}

/// Push a slide's own headings below the `## <slide title>` level (h1 → h3,
/// h2 → h4, capped at h6), or they read as slides of their own. Fenced code
/// is left alone.
fn demote_headings(md: &str) -> String {
    let mut fence: Option<(char, usize)> = None;
    let mut out = Vec::new();
    for line in md.lines() {
        let trimmed = line.trim_start_matches(' ');
        let indent = line.len() - trimmed.len();
        let mark = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
        if let (Some(c), true) = (mark, indent < 4) {
            let run = trimmed.chars().take_while(|x| *x == c).count();
            if run >= 3 {
                match fence {
                    None => fence = Some((c, run)),
                    Some((open, len))
                        if open == c && run >= len && trimmed[run..].trim().is_empty() =>
                    {
                        fence = None
                    }
                    Some(_) => {}
                }
                out.push(line.to_string());
                continue;
            }
        }
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        let is_heading = fence.is_none()
            && indent < 4
            && (1..=6).contains(&level)
            && trimmed[level..]
                .chars()
                .next()
                .is_none_or(|c| c == ' ' || c == '\t');
        if is_heading {
            let deeper = (level + 2).min(6);
            out.push(format!("{}{}", "#".repeat(deeper), &trimmed[level..]));
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// The stored workspace as links, then each small text file inline.
fn files_md(files: &[LinkedFile]) -> String {
    if files.is_empty() {
        return String::new();
    }
    let mut out = String::from("**Files**\n\n");
    for f in files {
        out.push_str(&format!(
            "- [{}]({})\n",
            link_text(&f.path),
            local_link(&f.rel)
        ));
    }
    for f in files {
        let Some(text) = &f.text else { continue };
        let fence = fence_for(text);
        out.push_str(&format!(
            "\n**{}**\n\n{fence}{}\n{}\n{fence}\n",
            link_text(&f.path),
            fence_lang(&f.path),
            text.trim_end_matches('\n')
        ));
    }
    out
}

/// A question, its answers as unticked boxes in Ed's order, and — only when
/// Ed sent a `solution`, which it does once solutions are released — the
/// answer and explanation, collapsed.
fn question_md(n: usize, q: &serde_json::Value) -> String {
    let data = &q["data"];
    let mut out = format!("### Question {n}\n\n");
    let content = ed_md(data["content"].as_str().unwrap_or(""));
    if !content.is_empty() {
        out.push_str(&content);
        out.push_str("\n\n");
    }
    let answers: Vec<String> = data["answers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(answer_line)
        .collect();
    for a in &answers {
        out.push_str(&format!("- [ ] {a}\n"));
    }

    let Some(solution) = data.get("solution") else {
        return out;
    };
    let chosen: Vec<String> = match solution {
        serde_json::Value::Array(items) => {
            items.iter().filter_map(|s| named(s, &answers)).collect()
        }
        single => named(single, &answers).into_iter().collect(),
    };
    let explanation = ed_md(data["explanation"].as_str().unwrap_or(""));
    if chosen.is_empty() && explanation.is_empty() {
        return out;
    }
    let mut body = String::new();
    match chosen.as_slice() {
        [] => {}
        [one] => body.push_str(&format!("**Answer:** {one}\n\n")),
        many => {
            body.push_str("**Answers:**\n\n");
            for a in many {
                body.push_str(&format!("- {a}\n"));
            }
            body.push('\n');
        }
    }
    body.push_str(&explanation);
    out.push_str(&format!(
        "\n<details>\n<summary>Solution</summary>\n\n{}\n\n</details>\n",
        body.trim()
    ));
    out
}

/// A solution entry: an index into the answers, or the answer itself (a
/// short-answer question's accepted text).
fn named(solution: &serde_json::Value, answers: &[String]) -> Option<String> {
    match solution {
        serde_json::Value::Number(n) => n.as_u64().and_then(|i| answers.get(i as usize)).cloned(),
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(answer_line(solution)),
        _ => None,
    }
}

/// An answer on one line: Ed sends each as `<document>` XML.
fn answer_line(answer: &serde_json::Value) -> String {
    let md = document_md(answer.as_str().unwrap_or(""));
    md.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A course-relative path under `lessons/` as a link from the lesson file's
/// folder, each segment percent-encoded, since a space or `(` would end a
/// Markdown link.
fn local_link(rel: &str) -> String {
    let within = rel.strip_prefix("lessons/").unwrap_or(rel);
    let mut out = String::new();
    for (i, segment) in within.split('/').enumerate() {
        if i > 0 {
            out.push('/');
        }
        for b in segment.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

fn link_text(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            let esc = matches!(c, '[' | ']' | '\\' | '*' | '_' | '`');
            esc.then_some('\\').into_iter().chain(std::iter::once(c))
        })
        .collect()
}

/// A backtick fence longer than any run inside the text.
fn fence_for(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

/// The code fence's language, from the file's extension.
fn fence_lang(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let Some((_, ext)) = name.rsplit_once('.') else {
        return String::new();
    };
    let ext = ext.to_ascii_lowercase();
    match ext.as_str() {
        "py" => "python",
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" => "typescript",
        "h" => "c",
        "hpp" | "cc" | "cxx" => "cpp",
        "hs" => "haskell",
        "md" => "markdown",
        "sh" => "bash",
        "yml" => "yaml",
        "ipynb" => "json",
        "txt" => "text",
        other => other,
    }
    .to_string()
}

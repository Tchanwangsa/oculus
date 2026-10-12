//! A slide's own content: Ed's XML, its workspace's files, a quiz's
//! questions, or a page through the HTML converter. A PDF slide has none:
//! the index links its PDF.

use super::super::document::document_md;
use super::links::{link_text, sibling_link};
use super::quiz::question_md;
use super::{LinkedFile, SlideExtras};

/// A slide's content as Markdown; `dir` is the slide file's folder,
/// course-relative, which its local links are relative to.
pub(super) fn slide_body(
    slide: &serde_json::Value,
    title: &str,
    extra: &SlideExtras,
    dir: &str,
) -> String {
    let text = |key: &str| slide[key].as_str().unwrap_or("");
    let mut parts: Vec<String> = Vec::new();
    match text("type") {
        "document" => parts.push(ed_md(text("content"))),
        "code" | "jupyter" => {
            parts.push(ed_md(text("content")));
            parts.push(files_md(&extra.files, dir));
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
pub(super) fn ed_md(xml: &str) -> String {
    demote_headings(&document_md(xml))
}

/// Push a slide's own headings two levels down (h1 → h3, h2 → h4, capped at
/// h6): below the file's `# <slide title>`, at the depth of a quiz's
/// `### Question`. Fenced code is left alone.
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
fn files_md(files: &[LinkedFile], dir: &str) -> String {
    if files.is_empty() {
        return String::new();
    }
    let mut out = String::from("**Files**\n\n");
    for f in files {
        out.push_str(&format!(
            "- [{}]({})\n",
            link_text(&f.path),
            sibling_link(&f.rel, dir)
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

//! One Ed lesson → an index file plus one Markdown file per slide. Pure:
//! whatever the sync fetched for a slide (quiz questions, the PDF and
//! workspace files it stored) arrives as [`SlideExtras`] on its [`SlideFile`].
//!
//! The index sits at `lessons/<lessonId>-<slug>.md` and links each slide's
//! file, `lessons/<lessonId>-<slug>/<NN>-<slide-slug>.md`; a slide's PDF and
//! workspace share its stem. A PDF slide has no file of its own, since its
//! stem's `.md` is where the parser writes the PDF's text: the index links
//! the PDF instead. Every local link is relative to the linking file's
//! folder. Both files open with front matter (`super::front`); the fields
//! are listed in `docs/sync.md`.

mod body;
mod links;
mod quiz;

use super::front::{front_matter, text_or_null};
use super::render::fmt_ts;
use body::slide_body;
use links::{link_text, local_link};

use serde_json::{json, Value};

/// What a slide links to or shows beyond its own JSON.
#[derive(Default)]
pub struct SlideExtras {
    /// The downloaded PDF, course-relative.
    pub pdf: Option<String>,
    pub questions: Vec<Value>,
    pub files: Vec<LinkedFile>,
    /// A challenge slide's `GET /challenges/<ch>` answer, when it came.
    pub challenge: Option<Value>,
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

/// A lesson's place in the library and in Ed.
pub struct LessonPlace<'a> {
    /// The index file's path without `.md`, course-relative:
    /// `lessons/<lessonId>-<slug>`. Its slides sit in the folder of that name.
    pub stem: &'a str,
    pub course_id: i64,
    pub lesson_id: i64,
    pub module: Option<&'a str>,
    /// Its position in the sidebar order (`lessons::order_lessons`), 0-based.
    pub order: usize,
    /// Ed's web region; no links while it is unknown.
    pub region: Option<&'a str>,
}

impl LessonPlace<'_> {
    pub fn url(&self) -> Option<String> {
        self.region
            .map(|r| lesson_url(r, self.course_id, self.lesson_id))
    }

    pub fn slide_url(&self, slide_id: i64) -> Option<String> {
        self.region
            .map(|r| slide_url(r, self.course_id, self.lesson_id, slide_id))
    }

    /// The folder the index sits in, with its trailing `/`.
    fn index_dir(&self) -> &str {
        self.stem.rfind('/').map_or("", |i| &self.stem[..=i])
    }

    /// The folder the slide files sit in, with its trailing `/`.
    fn slide_dir(&self) -> String {
        format!("{}/", self.stem)
    }
}

/// One slide's file: the slide, where it goes, and what the sync fetched.
pub struct SlideFile<'a> {
    pub slide: &'a Value,
    pub id: i64,
    /// 1-based position among the lesson's slides, the `NN` of its files.
    pub index: usize,
    /// Its `.md` path without the extension, course-relative:
    /// `lessons/<lessonId>-<slug>/<NN>-<slide-slug>`.
    pub stem: String,
    pub extras: SlideExtras,
}

impl SlideFile<'_> {
    /// The Ed challenge behind a `code` or `jupyter` slide.
    pub fn challenge_id(&self) -> Option<i64> {
        challenge_id(self.slide)
    }

    /// Whether the slide gets its own `.md` ([`slide_md`]): every slide but
    /// a PDF.
    pub fn has_file(&self) -> bool {
        self.slide["type"] != "pdf"
    }

    /// Ed's title, or `Slide <NN>` for an untitled slide.
    pub fn title(&self) -> String {
        self.slide["title"]
            .as_str()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("Slide {}", self.index))
    }
}

/// A slide's workspace folder from its file's path (either relative form):
/// the slide's assets share its stem.
pub fn slide_workspace_dir(slide_md: &str) -> Option<&str> {
    slide_md.strip_suffix(".md").filter(|d| !d.is_empty())
}

/// The web app's page for a lesson in Ed's `region`.
pub fn lesson_url(region: &str, course_id: i64, lesson_id: i64) -> String {
    format!("https://edstem.org/{region}/courses/{course_id}/lessons/{lesson_id}")
}

/// The web app's page for one slide of a lesson in Ed's `region`.
pub fn slide_url(region: &str, course_id: i64, lesson_id: i64, slide_id: i64) -> String {
    format!(
        "{}/slides/{slide_id}",
        lesson_url(region, course_id, lesson_id)
    )
}

/// The Ed challenge behind a slide: only `code` and `jupyter` slides have one.
pub fn challenge_id(slide: &Value) -> Option<i64> {
    match slide["type"].as_str() {
        Some("code" | "jupyter") => slide["challenge_id"].as_i64(),
        _ => None,
    }
}

/// What a workspace view needs from a challenge, without the student's
/// standing (`score`, `status`, `attempts_remaining`), which goes stale. A
/// challenge with no `due_at` is due with its lesson.
fn challenge_front(challenge: &Value, lesson: &Value) -> Value {
    let feature = |name: &str| json!(challenge["features"][name].as_bool());
    let due = match text_or_null(&challenge["due_at"]) {
        Value::Null => text_or_null(&lesson["due_at"]),
        due => due,
    };
    json!({
        "type": text_or_null(&challenge["type"]),
        "run_command": text_or_null(&challenge["settings"]["run_command"]),
        "features": {
            "run": feature("run"),
            "mark": feature("mark"),
            "terminal": feature("terminal"),
            "feedback": feature("feedback"),
            "confirm_submit": feature("confirm_submit"),
        },
        "due_at": due,
    })
}

/// A lesson's slides by `index`, then id.
pub fn sorted_slides(lesson: &Value) -> Vec<&Value> {
    let mut slides: Vec<&Value> = lesson["slides"].as_array().into_iter().flatten().collect();
    slides.sort_by_key(|s| {
        (
            s["index"].as_i64().unwrap_or(i64::MAX),
            s["id"].as_i64().unwrap_or(i64::MAX),
        )
    });
    slides
}

/// The lesson's index: front matter listing every slide, then the lesson's
/// title, module, dates and Ed link, then a numbered link to each slide file
/// (a PDF slide's PDF).
pub fn lesson_index_md(lesson: &Value, place: &LessonPlace, slides: &[SlideFile]) -> String {
    let title = lesson["title"].as_str().unwrap_or("Lesson");
    let url = place.url();
    let index_dir = place.index_dir();

    let entries: Vec<Value> = slides
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "title": s.title(),
                "type": text_or_null(&s.slide["type"]),
                "status": text_or_null(&s.slide["status"]),
                "file": s.has_file().then(|| within(&format!("{}.md", s.stem), index_dir)),
                "pdf": s.extras.pdf.as_deref().map(|p| within(p, index_dir)),
                "url": place.slide_url(s.id),
                "challenge_id": s.challenge_id(),
            })
        })
        .collect();
    let front = front_matter(&[
        ("ed_lesson_id", json!(place.lesson_id)),
        ("title", json!(title)),
        ("lesson_type", text_or_null(&lesson["type"])),
        ("module", json!(place.module.filter(|m| !m.is_empty()))),
        ("order", json!(place.order)),
        ("state", text_or_null(&lesson["state"])),
        ("available_at", text_or_null(&lesson["available_at"])),
        ("due_at", text_or_null(&lesson["due_at"])),
        ("locks_at", text_or_null(&lesson["locked_at"])),
        ("url", json!(url)),
        ("slides", Value::Array(entries)),
    ]);

    let mut md = format!("# {title}\n\n");
    if let Some(module) = place.module.filter(|m| !m.is_empty()) {
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
    if let Some(url) = &url {
        md.push_str(&format!("[Open in Ed]({url})\n"));
    }
    md.push('\n');

    for s in slides {
        let title = link_text(&s.title());
        let link = if s.has_file() {
            Some(local_link(&format!("{}.md", s.stem), index_dir))
        } else {
            match &s.extras.pdf {
                Some(pdf) => Some(local_link(pdf, index_dir)),
                None => s.slide["file_url"]
                    .as_str()
                    .filter(|u| !u.is_empty())
                    .map(str::to_string),
            }
        };
        match link {
            Some(link) => md.push_str(&format!("{}. [{title}]({link})", s.index)),
            None => md.push_str(&format!("{}. {title}", s.index)),
        }
        if let Some(kind) = s.slide["type"].as_str().filter(|k| !k.is_empty()) {
            md.push_str(&format!(" · {kind}"));
        }
        if let (false, Some(note)) = (s.has_file(), &s.extras.note) {
            md.push_str(&format!(" · _{note}_"));
        }
        md.push('\n');
    }

    front + &finish(&md)
}

/// One slide's file: front matter, its title, then its content. `None` for
/// a PDF slide, which has no file ([`SlideFile::has_file`]).
pub fn slide_md(lesson: &Value, place: &LessonPlace, slide: &SlideFile) -> Option<String> {
    if !slide.has_file() {
        return None;
    }
    let title = slide.title();
    let front = front_matter(&[
        ("ed_slide_id", json!(slide.id)),
        ("ed_lesson_id", json!(place.lesson_id)),
        ("ed_challenge_id", json!(slide.challenge_id())),
        (
            "lesson",
            json!(lesson["title"].as_str().unwrap_or("Lesson")),
        ),
        ("title", json!(title)),
        ("type", text_or_null(&slide.slide["type"])),
        ("status", text_or_null(&slide.slide["status"])),
        ("index", json!(slide.index)),
        // Only a PDF slide has one, and it has no file.
        ("pdf", Value::Null),
        ("url", json!(place.slide_url(slide.id))),
        (
            "challenge",
            match (&slide.extras.challenge, slide.challenge_id()) {
                (Some(challenge), Some(_)) => challenge_front(challenge, lesson),
                _ => Value::Null,
            },
        ),
    ]);
    let body = slide_body(slide.slide, &title, &slide.extras, &place.slide_dir());
    let open = place
        .slide_url(slide.id)
        .map(|url| format!("[Open in Ed]({url})\n\n"))
        .unwrap_or_default();
    Some(front + &finish(&format!("# {title}\n\n{open}{body}\n")))
}

/// `path` relative to `dir`, as written in front matter (not encoded).
fn within(path: &str, dir: &str) -> String {
    path.strip_prefix(dir).unwrap_or(path).to_string()
}

fn finish(md: &str) -> String {
    crate::pages::md::collapse_blank_lines(md)
        .trim()
        .to_string()
        + "\n"
}

//! Naming the chapters: the agent job.
//!
//! A coding agent is handed paths (outline, frames) and reads what it needs.
//! It never touches the database: it replies with JSON, which Rust parses,
//! validates and writes — chapters are derived data, so there is no write door.

use super::transcript::TranscriptCue;
use super::Candidate;

/// One named span of a recording. It ends where the next begins (the last at
/// the lecture's duration), so no end is stored.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Chapter {
    pub start_seconds: u32,
    pub title: String,
    pub summary: String,
}

/// What the agent replies with, before validation. The aliases accept the
/// synonyms models write despite the prompt.
#[derive(serde::Deserialize)]
struct ReplyChapter {
    #[serde(alias = "start_seconds", alias = "seconds", alias = "at")]
    start: f64,
    title: String,
    #[serde(default)]
    summary: String,
}

/// A reply that wrapped the array in an object.
#[derive(serde::Deserialize)]
struct ReplyEnvelope {
    chapters: Vec<ReplyChapter>,
}

/// More than this and it is a slide list, not a shape.
pub const MAX_CHAPTERS: usize = 12;

/// What the chaptering prompt needs; the agent reads the rest off disk.
pub struct Job<'a> {
    pub title: &'a str,
    pub duration_secs: u32,
    /// Relative to the agent's working directory (`agents/`).
    pub lecture_dir: &'a str,
    /// The subject's course folder, same relative shape. An Echo360 title is a
    /// room booking, so this is how the agent finds the deck.
    pub course_dir: Option<&'a str>,
    /// How many slide changes the outline marks.
    pub detected: usize,
    /// Whether the outline has any transcript in it.
    pub has_transcript: bool,
}

/// The transcript and the detected slide changes merged into one document in
/// play order, written to `<lecture dir>/outline.md` for the agent to read.
///
/// Every line is `second  timestamp  text`: the bare second is what a chapter's
/// `start` must be exactly, because a model converting a clock will round.
pub fn outline(title: &str, cues: &[TranscriptCue], changes: &[Candidate]) -> String {
    fn line(out: &mut String, second: u32, text: &str) {
        out.push_str(&format!("{second:>7}  {}  {text}\n", hms(second)));
    }
    fn marker(change: &Candidate) -> String {
        format!(
            "--- slide change · score {:.1}{} ---",
            change.score,
            if change.pause { " · pause" } else { "" }
        )
    }

    let mut out = format!(
        "# {title}\n\nEvery line is `second  timestamp  text`: a transcript cue, or a marker for a second where the slide changed. The two columns are the same moment; the first one is what a chapter start has to be.\n\n"
    );
    let mut marks = changes.iter().peekable();
    for cue in cues {
        let at = cue.start.max(0.0) as u32;
        while marks.peek().is_some_and(|m| m.seconds <= at) {
            let change = marks.next().expect("peeked");
            line(&mut out, change.seconds, &marker(change));
        }
        line(&mut out, at, &cue.text);
    }
    // Markers after the last cue (or all of them, with no transcript).
    for change in marks {
        line(&mut out, change.seconds, &marker(change));
    }
    out
}

/// The prompt for one chaptering turn. Three lines in it are load-bearing: a
/// slide change is not a chapter, the AV splash screen is not a slide, and the
/// `grep` that saves reading the whole outline.
pub fn prompt(job: &Job) -> String {
    format!(
        "Chapter a university lecture recording: choose its real topic boundaries and name \
each one.\n\n\
Lecture: {title}\n\
Duration: {clock} ({mins} minutes)\n\
Recording folder: {dir}\n  \
{dir}/outline.md           what was said and where the slides changed, merged, in play order\n  \
{dir}/frames/<second>.jpg  one grab per slide change, named by its second\n\
{course}\n\
The outline is the whole lecture as one document. Every line is `second  timestamp  text`: \
either a transcript cue, or a `--- slide change ---` marker for a second where the picture \
changed hard enough to be a new slide, carrying how hard it changed and whether the lecturer \
paused there. {detected} slide changes were found — \
`grep -n \"slide change\" {dir}/outline.md` lists them with the lines to read \
around.{silent}\n\n\
A slide change is a place you *may* cut, not a place you should: most of them are the same \
topic carrying on. And a topic can turn where no slide changed at all, so a boundary may be \
any line in the outline, marker or cue.\n\n\
How to work\n\
- Read the outline across a second you are considering and see whether the subject actually \
turns there. That is the evidence; the frames are corroboration. It is a long file — read \
the spans you want rather than all of it.\n\
- Open the frames you are unsure about. A title slide or a section divider usually starts a \
chapter; the next bullet of the same argument does not. View a handful as images — they are \
pictures of slides, so looking at them is the point; do not hash them or reach for an OCR \
tool.\n\
- Some frames are the lecture theatre's own AV splash screen — a \
room-control panel saying something like \"connect your laptop\" — and not a \
slide at all. They survive when the recording dropped out for a while. Ignore \
them completely and never name a chapter after one.\n\n\
Rules\n\
- Give the lecture as many chapters as it genuinely has: usually 5 to 8, never \
more than {max}. Do not pad a coherent fifteen-minute stretch into three \
chapters, and do not merge two genuinely different topics to keep the list \
short.\n\
- A chapter shorter than about three minutes is a slide, not a topic: fold it \
into whichever neighbour it belongs to. But do not let that swallow a real \
segment — a long stretch of housekeeping, a worked example or a Q&A is its own \
chapter if it lasts.\n\
- The first chapter starts at second 0.\n\
- Every other start must be exactly one of the seconds in the outline's first column. Do not \
round one, and do not pick a second between two lines.\n\
- Titles name the topic in the lecturer's own vocabulary, two to six words, \
sentence case: \"Grover's search\", \"Proving unsatisfiability by resolution\". \
Never \"Introduction\", \"Part 2\", \"Continued\", \"Wrap-up\", and never the \
lecture's own title.\n\
- Summaries are one or two sentences saying what is covered and why a student \
would come back to this span. Say something the title does not — a summary \
that restates its title is worth nothing.\n\n\
Reply with JSON and nothing else, in play order:\n\n\
[\n  {{\"start\": 0, \"title\": \"...\", \"summary\": \"...\"}},\n  \
{{\"start\": 742, \"title\": \"...\", \"summary\": \"...\"}}\n]\n",
        title = job.title,
        course = job
            .course_dir
            .map(|d| format!("  {d}/   the subject's own materials, including the slide deck\n"))
            .unwrap_or_default(),
        clock = hms(job.duration_secs),
        mins = job.duration_secs / 60,
        dir = job.lecture_dir,
        detected = job.detected,
        silent = if job.has_transcript {
            ""
        } else {
            " This recording has no transcript, so the outline is those markers and nothing else."
        },
        max = MAX_CHAPTERS,
    )
}

/// `HH:MM:SS`, as every recording prompt prints it beside the bare second.
pub fn hms(secs: u32) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// The chapter array out of whatever the agent said. Whether it is allowed is
/// [`validate`]'s question.
pub fn parse_chapters(reply: &str) -> Result<Vec<Chapter>, String> {
    parse_reply(reply, "chapter list", decode)
}

/// The first fragment of `reply` that `decode` accepts, trying the whole
/// reply, each fenced block, then balanced `[…]`/`{…}` runs — models wrap JSON
/// in prose, fences or objects however plainly asked. `what` names the
/// expected shape in the error. Shared with `lecture_end`.
pub(crate) fn parse_reply<T>(
    reply: &str,
    what: &str,
    decode: impl Fn(&str) -> Option<T>,
) -> Result<T, String> {
    for candidate in json_candidates(reply) {
        if let Some(parsed) = decode(&candidate) {
            return Ok(parsed);
        }
    }
    Err(format!(
        "no {what} in the reply ({} chars): {}",
        reply.chars().count(),
        clip(reply.trim(), 200)
    ))
}

fn decode(text: &str) -> Option<Vec<Chapter>> {
    let items: Vec<ReplyChapter> = serde_json::from_str(text)
        .or_else(|_| serde_json::from_str::<ReplyEnvelope>(text).map(|e| e.chapters))
        .ok()?;
    if items.is_empty() {
        return None;
    }
    Some(
        items
            .into_iter()
            .map(|c| Chapter {
                start_seconds: c.start.max(0.0).round() as u32,
                title: c.title.trim().to_string(),
                summary: c.summary.trim().to_string(),
            })
            .collect(),
    )
}

/// Fragments of `reply` worth trying to parse, most likely first.
fn json_candidates(reply: &str) -> Vec<String> {
    let mut out = vec![reply.trim().to_string()];
    // Fenced blocks; the opening fence's info string is dropped with its line.
    let mut rest = reply;
    while let Some(open) = rest.find("```") {
        let after = &rest[open + 3..];
        let body = match after.find('\n') {
            Some(nl) => &after[nl + 1..],
            None => break,
        };
        match body.find("```") {
            Some(close) => {
                out.push(body[..close].trim().to_string());
                rest = &body[close + 3..];
            }
            None => {
                out.push(body.trim().to_string());
                break;
            }
        }
    }
    for open in ['[', '{'] {
        out.extend(balanced_runs(reply, open));
    }
    out
}

/// Up to eight balanced `open`…`close` spans of `text`, in order — several,
/// because prose can hold brackets of its own. String literals are respected.
fn balanced_runs(text: &str, open: char) -> Vec<String> {
    const BALANCED_RUNS: usize = 8;
    let close = if open == '[' { ']' } else { '}' };
    let mut out: Vec<String> = Vec::new();
    let mut from = 0usize;
    while out.len() < BALANCED_RUNS {
        let Some(offset) = text[from..].find(open) else {
            break;
        };
        let start = from + offset;
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        let mut end: Option<usize> = None;
        for (at, ch) in text[start..].char_indices() {
            if in_string {
                match ch {
                    _ if escaped => escaped = false,
                    '\\' => escaped = true,
                    '"' => in_string = false,
                    _ => {}
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                c if c == open => depth += 1,
                c if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + at + ch.len_utf8());
                        break;
                    }
                }
                _ => {}
            }
        }
        match end {
            Some(e) => {
                out.push(text[start..e].to_string());
                from = e;
            }
            None => break,
        }
    }
    out
}

fn clip(text: &str, chars: usize) -> String {
    let head: String = text.chars().take(chars).collect();
    if text.chars().count() > chars {
        format!("{head}…")
    } else {
        head.to_string()
    }
}

/// Whether a parsed chapter set may be written. One bad chapter rejects the
/// whole set — dropping one would let its neighbour silently swallow its span.
///
/// `boundaries` is every second the outline printed (0, the slide changes and
/// every cue start): wide enough to chapter from speech alone, but closed, so
/// the agent cannot invent a timestamp.
pub fn validate(
    chapters: &[Chapter],
    boundaries: &[u32],
    duration_secs: u32,
) -> Result<(), String> {
    if chapters.is_empty() {
        return Err("no chapters in the reply".to_string());
    }
    if chapters.len() > MAX_CHAPTERS {
        return Err(format!(
            "{} chapters is more than the {MAX_CHAPTERS} allowed — that is a slide list, not a shape",
            chapters.len()
        ));
    }
    if chapters[0].start_seconds != 0 {
        return Err(format!(
            "chapter 1 ({:?}): the first chapter must start at 0, not {}",
            chapters[0].title, chapters[0].start_seconds
        ));
    }
    let mut previous: Option<u32> = None;
    for (n, chapter) in chapters.iter().enumerate() {
        let where_ = format!("chapter {} ({:?}): ", n + 1, chapter.title);
        if chapter.title.is_empty() {
            return Err(format!("chapter {}: a chapter needs a title", n + 1));
        }
        if chapter.summary.is_empty() {
            return Err(format!("{where_}a chapter needs a summary"));
        }
        if !boundaries.contains(&chapter.start_seconds) {
            return Err(format!(
                "{where_}{} is not one of the seconds in the outline",
                chapter.start_seconds
            ));
        }
        if duration_secs > 0 && chapter.start_seconds >= duration_secs {
            return Err(format!(
                "{where_}starts at {}, past the end of a {duration_secs}s recording",
                chapter.start_seconds
            ));
        }
        if let Some(p) = previous {
            if chapter.start_seconds <= p {
                return Err(format!(
                    "{where_}starts at {}, which is not after the chapter before it ({p})",
                    chapter.start_seconds
                ));
            }
        }
        previous = Some(chapter.start_seconds);
    }
    Ok(())
}

//! `oculus studio lint`: `Project::lint` (or `lint_scene`) plus the checks
//! that need the narration engine (`voice::lint`) or the library (a slide
//! whose PDF changed, `slides::lint`), printed short: errors one
//! a line in full, then warnings and notes, a scene's repeats of one kind
//! on one line with a count and the first few.

use super::studio_error;
use crate::*;
use app_lib::studio::tts::kokoro::{self, LexiconFiles};
use app_lib::studio::voice::lint::{Note, VoiceLint};
use app_lib::studio::voice::review::Store;
use app_lib::studio::voice::{self, VoiceOverrides};
use app_lib::studio::{brief, libraries, slides};
use studio_core::{LintIssue, LintLevel, Project};

/// Examples a grouped line shows before `…`.
const GROUP_EXAMPLES: usize = 3;

impl Ctx {
    pub(crate) fn studio_lint(&self, args: &StudioLintArgs) -> Result<(), String> {
        let project = libraries::open(&args.dir, None, None).map_err(studio_error)?;
        let scope = match &args.scene {
            Some(s) => Some(brief::scene_index(&project, s)?),
            None => None,
        };
        let mut report = match scope {
            Some(i) => project.lint_scene(i),
            None => project.lint(),
        }
        .map_err(studio_error)?;
        let mut spoken = voice_lint(&project);
        if let Some(i) = scope {
            spoken.issues.retain(|x| x.scene.is_none_or(|s| s == i));
            spoken.notes.retain(|n| n.scene == i);
        }
        Store::app().annotate(project.video.subject.as_deref(), &mut spoken.notes);
        report.issues.extend(spoken.issues);
        let mut stale = slides::lint(&project, &self.data_dir);
        if let Some(i) = scope {
            stale.retain(|x| x.scene.is_none_or(|s| s == i));
        }
        report.issues.extend(stale);
        let errors = report.errors();

        if self.json {
            let entries: Vec<serde_json::Value> = report
                .issues
                .iter()
                .map(issue_json)
                .chain(spoken.notes.iter().map(note_json))
                .collect();
            self.emit(&entries)?;
        } else {
            print_report(&report.issues, &spoken.notes);
            let rest = report.issues.len() - errors + spoken.notes.len();
            if errors == 0 {
                let what = match scope {
                    Some(i) => format!("{} scene {i}", args.dir.display()),
                    None => format!(
                        "{} ({} scenes, {:.3} s)",
                        args.dir.display(),
                        project.video.scenes.len(),
                        project.length().map_err(studio_error)?
                    ),
                };
                let head = if rest == 0 {
                    "no problems"
                } else {
                    "no errors"
                };
                println!(
                    "{} {what}{}",
                    paint(head, GREEN),
                    counts(&report.issues, &spoken.notes)
                );
            }
        }
        if errors == 0 {
            return Ok(());
        }
        Err(format!(
            "{errors} error{} in {}",
            if errors == 1 { "" } else { "s" },
            args.dir.display()
        ))
    }
}

/// ` · 2 warnings, 5 notes`, or nothing.
fn counts(issues: &[LintIssue], notes: &[Note]) -> String {
    let warnings = issues
        .iter()
        .filter(|i| i.level == LintLevel::Warning)
        .count();
    let n = issues.iter().filter(|i| i.level == LintLevel::Note).count() + notes.len();
    let mut parts = Vec::new();
    if warnings > 0 {
        parts.push(plural(warnings, "warning"));
    }
    if n > 0 {
        parts.push(plural(n, "note"));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!(" · {}", parts.join(", "))
}

fn plural(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}

/// A warning or note in a scene, ready to group: its scene, level, noun,
/// short form and full line.
struct Entry {
    scene: Option<usize>,
    place: String,
    level: &'static str,
    noun: &'static str,
    short: String,
    full: String,
}

fn print_report(issues: &[LintIssue], notes: &[Note]) {
    for issue in issues.iter().filter(|i| i.level == LintLevel::Error) {
        print_issue(issue);
    }
    let mut entries: Vec<Entry> = Vec::new();
    for level in [LintLevel::Warning, LintLevel::Note] {
        for issue in issues.iter().filter(|i| i.level == level) {
            let (noun, short) = issue.group.clone().unwrap_or(("issue", String::new()));
            entries.push(Entry {
                scene: issue.scene,
                place: place(issue).unwrap_or_else(|| "video".to_owned()),
                level: level.name(),
                noun,
                short,
                full: issue.error.to_string(),
            });
        }
        if level == LintLevel::Note {
            entries.extend(notes.iter().map(|n| Entry {
                scene: Some(n.scene),
                place: format!("scene {} `{}`", n.scene, n.id),
                level: "note",
                noun: "guessed word",
                short: match &n.review {
                    Some(r) => format!("\"{}\" script.md:{} {}", n.word, n.line, r.suffix()),
                    None => format!("\"{}\" script.md:{}", n.word, n.line),
                },
                full: format!("script.md:{}: {}", n.line, n.message()),
            }));
        }
    }
    let mut done = vec![false; entries.len()];
    for i in 0..entries.len() {
        if done[i] {
            continue;
        }
        let e = &entries[i];
        let same: Vec<usize> = (i..entries.len())
            .filter(|&j| {
                let o = &entries[j];
                !done[j] && o.scene == e.scene && o.level == e.level && o.noun == e.noun
            })
            .collect();
        for &j in &same {
            done[j] = true;
        }
        let label = paint(&format!("{}:", e.level), BOLD);
        // A warning keeps its words: they say where in the narration to fix it.
        if e.level == LintLevel::Warning.name() {
            for &j in &same {
                println!("{label} {}: {}", paint(&e.place, BOLD), entries[j].full);
            }
            continue;
        }
        if same.len() == 1 {
            println!("{label} {}: {}", paint(&e.place, BOLD), e.full);
            continue;
        }
        let shown: Vec<&str> = same
            .iter()
            .take(GROUP_EXAMPLES)
            .map(|&j| entries[j].short.as_str())
            .collect();
        let more = if same.len() > GROUP_EXAMPLES {
            ", …"
        } else {
            ""
        };
        println!(
            "{label} {}: {} {}s: {}{more}",
            paint(&e.place, BOLD),
            same.len(),
            e.noun,
            shown.join(", ")
        );
    }
}

/// The checks that need the narration engine (`voice::lint`), with the
/// voice `narrate` would resolve without flags. A voice that does not
/// resolve is `narrate`'s to report.
fn voice_lint(project: &Project) -> VoiceLint {
    let block = project
        .video
        .voice
        .as_ref()
        .map(VoiceOverrides::from)
        .unwrap_or_default();
    match voice::resolve(&VoiceOverrides::default(), &block, &voice::settings()) {
        Ok(v) => voice::lint::lint(
            project,
            &v,
            &kokoro::dir(),
            &LexiconFiles::app(project.video.subject.as_deref()),
        ),
        Err(_) => VoiceLint::default(),
    }
}

fn note_json(note: &Note) -> serde_json::Value {
    serde_json::json!({
        "level": "note",
        "scene": note.scene,
        "id": note.id,
        "line": note.line,
        "kind": "guessed",
        "message": note.message(),
        "word": note.word,
        "phonemes": note.phonemes,
        "guessed": note.guessed,
        "review": note.review,
    })
}

/// Where an issue is (`scene 2 \`update\` (Steps)`), or `None` when the
/// error names its own place (`script.md:7: ...`).
fn place(issue: &LintIssue) -> Option<String> {
    let id = issue
        .id
        .as_ref()
        .map(|id| format!(" `{id}`"))
        .unwrap_or_default();
    let component = issue.component.as_deref();
    Some(match (issue.scene, &issue.example, component) {
        (Some(i), _, Some(c)) => format!("scene {i}{id} ({c})"),
        (Some(i), _, None) => format!("scene {i}{id}"),
        (None, Some(e), Some(c)) => format!("{c} example `{e}`"),
        (None, _, Some(c)) => c.to_owned(),
        (None, _, None) => return None,
    })
}

/// One error: where it is, then what is wrong.
fn print_issue(issue: &LintIssue) {
    match place(issue) {
        Some(p) => println!("{} {}", paint(&p, BOLD), issue.error),
        None => println!("{}", issue.error),
    }
}

fn issue_json(issue: &LintIssue) -> serde_json::Value {
    let e = &issue.error;
    serde_json::json!({
        "level": issue.level.name(),
        "scene": issue.scene,
        "id": issue.id,
        "component": issue.component,
        "example": issue.example,
        "kind": e.kind(),
        "message": e.to_string(),
        "loc": e.loc().map(|l| serde_json::json!({
            "line": l.line,
            "col": l.col,
            "snippet": l.snippet.trim(),
        })),
    })
}

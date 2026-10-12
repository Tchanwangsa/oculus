//! The narration's lint: words the aligner was unsure of (a mangled word
//! smears its neighbours' timing, so each one is named, as a note), and
//! speech the script does not have.

use super::{Level, LintIssue, LintReport};
use crate::error::StudioError;
use crate::project::{LOW_CONFIDENCE, Project};

/// Scene `index`'s narration problems. Missing or stale narration is
/// reported with the scene's length, not here.
pub(super) fn lint(project: &Project, index: usize, report: &mut LintReport) {
    let (Ok(Some(clip)), Some(section)) = (project.narration(index), project.section(index)) else {
        return;
    };
    let issue = |message: String| {
        let scene = section.id.clone();
        let error = StudioError::Narration { scene, message };
        LintIssue::scene(project, index, error)
    };
    let lead = project.theme.narration.lead;
    for w in clip.words.iter().filter(|w| w.confidence < LOW_CONFIDENCE) {
        let at = w.start + lead;
        let mut note = issue(format!(
            "the aligner is unsure of `{}` at {at:.2} s (confidence {:.2}, below {LOW_CONFIDENCE}); its timing and its neighbours' may be off: listen to it, and reword or re-narrate",
            w.text, w.confidence
        ));
        note.level = Level::Note;
        note.group = Some(("unsure word", format!("\"{}\" {at:.1} s", w.text)));
        report.issues.push(note);
    }
    for g in &clip.gaps {
        let place = match g.after.and_then(|a| clip.words.get(a)) {
            Some(w) => format!("after `{}`", w.text),
            None => "before the first word".to_owned(),
        };
        report.issues.push(issue(format!(
            "the audio has unscripted speech {place} (heard \"{}\")",
            g.heard
        )));
    }
}

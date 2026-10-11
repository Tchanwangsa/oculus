//! The narration's lint: a clip too long to align, words the aligner was
//! unsure of (a mangled word smears its neighbours' timing, so each one is
//! named), and speech the script does not have.

use super::{LintIssue, LintReport};
use crate::error::StudioError;
use crate::project::{LOW_CONFIDENCE, MAX_NARRATION_SECS, Project};

/// Scene `index`'s narration problems. Missing or stale narration is
/// reported with the scene's length, not here.
pub(super) fn lint(project: &Project, index: usize, report: &mut LintReport) {
    let (Ok(Some(clip)), Some(section)) = (project.narration(index), project.section(index)) else {
        return;
    };
    let mut push = |message: String| {
        let scene = section.id.clone();
        let error = StudioError::Narration { scene, message };
        report.issues.push(LintIssue::scene(project, index, error));
    };
    if clip.duration > MAX_NARRATION_SECS {
        push(format!(
            "narration is {:.1} s; split the paragraph into scenes of at most {MAX_NARRATION_SECS} s",
            clip.duration
        ));
    }
    let lead = project.theme.narration.lead;
    for w in clip.words.iter().filter(|w| w.confidence < LOW_CONFIDENCE) {
        push(format!(
            "the aligner is unsure of `{}` at {:.2} s (confidence {:.2}, below {LOW_CONFIDENCE}); its timing and its neighbours' may be off: listen to it, and reword or re-narrate",
            w.text,
            w.start + lead,
            w.confidence
        ));
    }
    for g in &clip.gaps {
        let place = match g.after.and_then(|a| clip.words.get(a)) {
            Some(w) => format!("after `{}`", w.text),
            None => "before the first word".to_owned(),
        };
        push(format!(
            "the audio has unscripted speech {place} (heard \"{}\")",
            g.heard
        ));
    }
}

//! The script's lint: sections no scene matches, and text that is not
//! spoken form (`spoken`). Scenes with an id and no section are silent,
//! which needs a `hold`; `Project::scene_length` reports that.

use super::{LintIssue, LintReport, spoken};
use crate::error::StudioError;
use crate::project::Project;
use crate::suggest::did_you_mean;

pub(super) fn lint(project: &Project, report: &mut LintReport) {
    let Some(script) = &project.script else {
        return;
    };
    let scenes = &project.video.scenes;
    for section in &script.sections {
        let index = scenes
            .iter()
            .position(|s| s.id.as_deref() == Some(section.id.as_str()));
        let issue = |line: usize, message: String| {
            let error = StudioError::Script { line, message };
            match index {
                Some(i) => LintIssue::scene(project, i, error),
                None => LintIssue {
                    scene: None,
                    id: Some(section.id.clone()),
                    component: None,
                    example: None,
                    error,
                },
            }
        };
        if index.is_none() {
            let ids = scenes.iter().filter_map(|s| s.id.as_deref());
            report.issues.push(issue(
                section.line,
                format!(
                    "`## {}` matches no scene{}; give a scene in video.json this `id`, or remove the section",
                    section.id,
                    did_you_mean(&section.id, ids)
                ),
            ));
        }
        for u in spoken::scan(&section.raw) {
            report.issues.push(issue(
                section.line + 1 + u.line,
                format!("`{}` is not spoken form: {}", u.token, u.hints.join("; ")),
            ));
        }
    }
}

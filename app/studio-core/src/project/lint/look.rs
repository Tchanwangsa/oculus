//! Lint's warnings: how each scene that renders cleanly looks over time
//! (`crate::look`). A layout that fails mid-scene is an error, since the
//! two renders at the ends missed it.

use super::{Level, LintIssue, LintReport};
use crate::error::StudioError;
use crate::project::Project;

pub(super) fn lint(project: &Project, scenes: &[usize], report: &mut LintReport) {
    for (&index, look) in scenes.iter().zip(project.look(scenes)) {
        match look {
            Ok(look) => {
                for w in look.warnings() {
                    let mut issue = LintIssue::scene(
                        project,
                        index,
                        StudioError::Look {
                            kind: w.kind,
                            message: w.message,
                        },
                    );
                    issue.level = Level::Warning;
                    issue.group = Some((w.noun, w.short));
                    report.issues.push(issue);
                }
            }
            Err(error) => {
                let message = error.to_string();
                let known = report
                    .issues
                    .iter()
                    .any(|i| i.scene == Some(index) && message.contains(&i.error.to_string()));
                if !known {
                    report.issues.push(LintIssue::scene(project, index, error));
                }
            }
        }
    }
}

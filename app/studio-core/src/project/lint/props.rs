//! Required props: a prop with no `.default()` and not `.optional()` must
//! be in every `examples` entry, so the gallery can show it. With no
//! examples at all, every required prop is reported once.

use super::{Level, LintIssue, LintReport};
use crate::error::StudioError;
use crate::host::ComponentInfo;

pub(super) fn check(name: &str, info: &ComponentInfo, report: &mut LintReport) {
    let required: Vec<&str> = info
        .props
        .iter()
        .filter(|(_, spec)| spec.default.is_none() && !spec.optional)
        .map(|(prop, _)| prop.as_str())
        .collect();
    if required.is_empty() {
        return;
    }
    if info.examples.is_empty() {
        report.issues.push(LintIssue {
            level: Level::Error,
            group: None,
            scene: None,
            id: None,
            component: Some(name.to_owned()),
            example: None,
            error: StudioError::Lint(format!(
                "has required props ({}) but no `examples`; add one example that sets them all, so the gallery can show it",
                required.join(", ")
            )),
        });
        return;
    }
    for (example, value) in &info.examples {
        let Some(given) = value.as_object() else {
            continue;
        };
        for prop in &required {
            if !given.contains_key(*prop) {
                report.issues.push(LintIssue {
                    level: Level::Error,
                    group: None,
                    scene: None,
                    id: None,
                    component: Some(name.to_owned()),
                    example: Some(example.clone()),
                    error: StudioError::Lint(format!(
                        "omits required prop `{prop}`, which has no default; add it, or give the prop `.default(...)`"
                    )),
                });
            }
        }
    }
}

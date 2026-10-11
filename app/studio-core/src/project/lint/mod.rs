//! `lint`: load every component, check every scene's props and every
//! component example, render each scene at its first and last moment and
//! the middle of the transition into it, and check its transition and
//! motion values (no files written). For a narrated video it also checks
//! the script (`script`) and the narration (`narration`). Collects every
//! problem instead of stopping at the first, each tagged with its scene,
//! scene id and component.

mod narration;
mod props;
mod script;
mod spoken;
mod tokens;
mod tsx;

use super::{ComponentSource, Project, Staged};
use crate::error::{Result, StudioError};
use crate::render::{Engine, RenderOptions, scene_frame};
use crate::theme::Theme;
use crate::transition::{blur_mode, check_length};
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;

/// Lint renders small: layout is identical at any scale.
const LINT_SCALE: f64 = 0.25;

#[derive(Debug)]
pub struct LintIssue {
    /// The scene (0-based), or `None` for a problem in a component itself
    /// or in a script section no scene matches.
    pub scene: Option<usize>,
    /// The scene's `id`, or the script section's.
    pub id: Option<String>,
    /// The component, or `None` for a problem in the script alone.
    pub component: Option<String>,
    /// The `examples` entry this is about, if any.
    pub example: Option<String>,
    pub error: StudioError,
}

#[derive(Debug, Default)]
pub struct LintReport {
    pub issues: Vec<LintIssue>,
}

impl LintReport {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

impl LintIssue {
    /// An issue about scene `index` of `project`.
    fn scene(project: &Project, index: usize, error: StudioError) -> LintIssue {
        let spec = &project.video.scenes[index];
        LintIssue {
            scene: Some(index),
            id: spec.id.clone(),
            component: Some(spec.component.clone()),
            example: None,
            error,
        }
    }
}

/// Where the issue is (`scene 2 \`update\` (Steps)`, `Steps example
/// \`short\``), or nothing when the error names its own place.
impl fmt::Display for LintIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = self
            .id
            .as_ref()
            .map(|id| format!(" `{id}`"))
            .unwrap_or_default();
        let component = self.component.as_deref();
        match (self.scene, &self.example, component) {
            (Some(i), _, Some(c)) => write!(f, "scene {i}{id} ({c}): {}", self.error),
            (Some(i), _, None) => write!(f, "scene {i}{id}: {}", self.error),
            (None, Some(e), Some(c)) => write!(f, "{c} example `{e}`: {}", self.error),
            (None, _, Some(c)) => write!(f, "{c}: {}", self.error),
            (None, _, None) => write!(f, "{}", self.error),
        }
    }
}

impl fmt::Display for LintReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.issues.is_empty() {
            return write!(f, "no problems");
        }
        for issue in &self.issues {
            writeln!(f, "{issue}")?;
        }
        Ok(())
    }
}

impl Project {
    /// Errors here are about the project as a whole (the renderer could not
    /// start); everything about scenes, components, the script and the
    /// narration is in the report. A scene whose length is unknown (stale
    /// narration, no hold) is reported and not rendered.
    pub fn lint(&self) -> Result<LintReport> {
        let mut engine = Engine::new(
            self.theme.clone(),
            self.video.size,
            Some(self.assets_dir()),
            RenderOptions {
                scale: LINT_SCALE,
                motion_blur: None,
            },
        )?;
        let mut report = LintReport::default();
        script::lint(self, &mut report);
        let mut failed: HashSet<&str> = HashSet::new();
        let mut checked: HashSet<&str> = HashSet::new();
        // A theme or video.json value is reported once, not per scene.
        let mut transition_seen: HashSet<String> = HashSet::new();
        let transitions = self.transitions();
        let scenes = &self.video.scenes;
        let mut clean = vec![false; scenes.len()];
        let mut staged: Vec<Option<Staged>> = Vec::with_capacity(scenes.len());
        let video_level = [
            self.morph_color().map(|c| engine.set_morph_color(c)),
            blur_mode(&self.video.motion, &self.theme).map(|_| ()),
        ];
        for e in video_level.into_iter().filter_map(Result::err) {
            transition_seen.insert(e.to_string());
            report.issues.push(LintIssue::scene(self, 0, e.into()));
        }
        for (index, scene) in scenes.iter().enumerate() {
            let name = scene.component.as_str();
            let issue = |error| LintIssue::scene(self, index, error);
            let stage = self.staged(index);
            narration::lint(self, index, &mut report);
            let length = stage.as_ref().map_or(f64::INFINITY, |s| s.length);
            let stage = stage.map_err(|e| report.issues.push(issue(e))).ok();
            let into = transitions[index]
                .as_ref()
                .map_err(Clone::clone)
                .and_then(|t| {
                    check_length(index, scene.transition.as_ref(), t.as_ref(), length)?;
                    Ok(t.as_ref())
                });
            if let Err(e) = &into
                && transition_seen.insert(e.to_string())
            {
                report.issues.push(issue(e.clone().into()));
            }
            if checked.insert(name) {
                let loaded = self
                    .components
                    .read(name)
                    .and_then(|tsx| engine.load_component(name, &tsx).map(|_| tsx));
                match loaded {
                    Ok(tsx) => {
                        lint_examples(&engine, name, &mut report);
                        lint_source(&engine, name, &tsx, &self.theme, &mut report);
                    }
                    Err(error) => {
                        failed.insert(name);
                        report.issues.push(issue(error));
                    }
                }
            }
            let Some(st) = stage.filter(|_| !failed.contains(name)) else {
                staged.push(None);
                continue;
            };
            let end = st.length - (st.length / 2.0).min(1e-3);
            let mut seen: HashSet<String> = HashSet::new();
            let before = report.issues.len();
            for t in [0.0, end] {
                if let Err(error) = engine.render_checked(st.call(index, t))
                    && seen.insert(error.to_string())
                {
                    report.issues.push(issue(error));
                }
            }
            clean[index] = report.issues.len() == before;
            // The transition's middle, once both scenes render on their own.
            if let Ok(Some(tr)) = into
                && clean[index]
                && clean[index - 1]
                && tr.covers(0.0)
                && let Some(prev) = &staged[index - 1]
            {
                let mid = tr.dur.min(st.length) / 2.0;
                let a = prev.call(index - 1, 0.0);
                if let Err(error) = scene_frame(&mut engine, Some(a), st.call(index, mid), Some(tr))
                    && seen.insert(error.to_string())
                {
                    report.issues.push(issue(error));
                }
            }
            staged.push(Some(st));
        }
        Ok(report)
    }
}

/// The checks on a component's own source and declared props: token use
/// (`tokens`) and required props (`props`).
fn lint_source(engine: &Engine, name: &str, tsx: &str, theme: &Theme, report: &mut LintReport) {
    for error in tokens::check(name, tsx, theme) {
        report.issues.push(LintIssue {
            scene: None,
            id: None,
            component: Some(name.to_owned()),
            example: None,
            error,
        });
    }
    if let Some(info) = engine.component(name) {
        props::check(name, info, report);
    }
}

fn lint_examples(engine: &Engine, name: &str, report: &mut LintReport) {
    let Some(info) = engine.component(name) else {
        return;
    };
    for (example, props) in &info.examples {
        let result = match props {
            Value::Object(p) => engine.check_props(name, p).map(|_| ()),
            other => Err(StudioError::Component {
                component: name.to_owned(),
                message: format!(
                    "examples.{example} must be an object of props, got {}",
                    crate::scene::describe(other)
                ),
                loc: None,
            }),
        };
        if let Err(error) = result {
            report.issues.push(LintIssue {
                scene: None,
                id: None,
                component: Some(name.to_owned()),
                example: Some(example.clone()),
                error,
            });
        }
    }
}

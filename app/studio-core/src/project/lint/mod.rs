//! `lint`: load every component, check every scene's props and every
//! component example, render each scene at its first and last moment and
//! the middle of the transition into it, and check its transition and
//! motion values (no files written). For a narrated video it also checks
//! the script (`script`) and the narration (`narration`). Collects every
//! problem instead of stopping at the first, each tagged with its scene,
//! scene id and component.

mod look;
mod narration;
mod props;
mod script;
mod spoken;
mod tokens;
mod tsx;

use super::{ComponentSource, Project, Staged};
use crate::error::{Result, StudioError};
use crate::render::{Engine, RenderOptions, scene_frame};
use crate::schema::PropKind;
use crate::theme::Theme;
use crate::transition::{blur_mode, check_length};
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;

/// Lint renders small: layout is identical at any scale.
const LINT_SCALE: f64 = 0.25;

/// How much an issue matters. Only errors fail a lint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Stops a render, or breaks a rule: `oculus studio lint` exits 1.
    Error,
    /// How a scene looks over time (`look::warn`): worth a fix, never
    /// fails a lint.
    Warning,
    /// Worth a listen or a look, such as a word the aligner was unsure of.
    Note,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warning => "warning",
            Level::Note => "note",
        }
    }
}

#[derive(Debug)]
pub struct LintIssue {
    pub level: Level,
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
    /// A warning or note's name for one of its kind (`unsure word`) and
    /// itself in a few words, so repeats in a scene print as one line.
    pub group: Option<(&'static str, String)>,
}

#[derive(Debug, Default)]
pub struct LintReport {
    pub issues: Vec<LintIssue>,
    /// How many of the first issues are about the video's own settings.
    video: usize,
}

impl LintReport {
    /// No errors; warnings and notes may remain.
    pub fn is_clean(&self) -> bool {
        self.errors() == 0
    }

    pub fn errors(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.level == Level::Error)
            .count()
    }
}

impl LintIssue {
    /// An issue about scene `index` of `project`.
    fn scene(project: &Project, index: usize, error: StudioError) -> LintIssue {
        let spec = &project.video.scenes[index];
        LintIssue {
            level: Level::Error,
            group: None,
            scene: Some(index),
            id: spec.id.clone(),
            component: Some(spec.component.clone()),
            example: None,
            error,
        }
    }
}

/// Where the issue is (`scene 2 \`update\` (Steps)`, `Steps example
/// \`short\``), or nothing when the error names its own place; a warning
/// or note starts with its level.
impl fmt::Display for LintIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = self
            .id
            .as_ref()
            .map(|id| format!(" `{id}`"))
            .unwrap_or_default();
        let component = self.component.as_deref();
        if self.level != Level::Error {
            write!(f, "{}: ", self.level.name())?;
        }
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
        self.lint_in(None)
    }

    /// `lint` of scene `index` alone: its own issues, its component's, and
    /// the video's settings that would stop it rendering. The scene before
    /// it is rendered too, for the transition, but not reported.
    pub fn lint_scene(&self, index: usize) -> Result<LintReport> {
        self.scene(index)?;
        let mut report = self.lint_in(Some(index))?;
        let component = self.video.scenes[index].component.as_str();
        let video = std::mem::take(&mut report.video);
        report.issues = report
            .issues
            .into_iter()
            .enumerate()
            .filter(|(i, issue)| {
                *i < video
                    || issue.scene == Some(index)
                    || (issue.scene.is_none() && issue.component.as_deref() == Some(component))
            })
            .map(|(_, issue)| issue)
            .collect();
        Ok(report)
    }

    fn lint_in(&self, scope: Option<usize>) -> Result<LintReport> {
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
        report.video = report.issues.len();
        script::lint(self, &mut report);
        // Scoped, only the scene and the one before it (for the transition).
        let wanted = |i: usize| scope.is_none_or(|s| i == s || i + 1 == s);
        for (index, scene) in scenes.iter().enumerate() {
            if !wanted(index) {
                staged.push(None);
                continue;
            }
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
                && scope.is_none_or(|s| s == index)
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
                        // Scoped, the scene before is not reported: let
                        // the scene itself load it again and report it.
                        if scope == Some(index + 1) {
                            checked.remove(name);
                        }
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
        let looked: Vec<usize> = (0..scenes.len())
            .filter(|&i| clean[i] && scope.is_none_or(|s| s == i))
            .collect();
        look::lint(self, &looked, &mut report);
        Ok(report)
    }
}

/// The checks on a component's own source and declared props: token use
/// (`tokens`, with its monospace note) and required props (`props`).
fn lint_source(engine: &Engine, name: &str, tsx: &str, theme: &Theme, report: &mut LintReport) {
    let colour_props: Vec<String> = engine.component(name).map_or_else(Vec::new, |info| {
        info.props
            .iter()
            .filter(|(_, spec)| matches!(&spec.kind, PropKind::Token(k) if k == "color"))
            .map(|(prop, _)| prop.clone())
            .collect()
    });
    for error in tokens::check(name, tsx, theme, &colour_props) {
        report.issues.push(LintIssue {
            level: Level::Error,
            group: None,
            scene: None,
            id: None,
            component: Some(name.to_owned()),
            example: None,
            error,
        });
    }
    if let Some(error) = tokens::mono_note(name, tsx) {
        report.issues.push(LintIssue {
            level: Level::Note,
            group: Some(("monospace outside code", name.to_owned())),
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
                level: Level::Error,
                group: None,
                scene: None,
                id: None,
                component: Some(name.to_owned()),
                example: Some(example.clone()),
                error,
            });
        }
    }
}

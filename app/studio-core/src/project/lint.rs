//! `lint`: load every component, check every scene's props and every
//! component example, and render each scene at its first and last moment
//! (no files written). Collects every problem instead of stopping at the
//! first, each tagged with its scene and component.

use super::Project;
use crate::error::{Result, StudioError};
use crate::render::{Engine, RenderOptions, SceneCall};
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;

/// Lint renders small: layout is identical at any scale.
const LINT_SCALE: f64 = 0.25;

#[derive(Debug)]
pub struct LintIssue {
    /// The scene (0-based), or `None` for a problem in a component itself.
    pub scene: Option<usize>,
    pub component: String,
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

impl fmt::Display for LintIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.scene, &self.example) {
            (Some(i), _) => write!(f, "scene {i} ({}): {}", self.component, self.error),
            (None, Some(e)) => write!(f, "{} example `{e}`: {}", self.component, self.error),
            (None, None) => write!(f, "{}: {}", self.component, self.error),
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
    /// start); everything about scenes and components is in the report.
    pub fn lint(&self) -> Result<LintReport> {
        let mut engine = Engine::new(
            self.theme.clone(),
            self.video.size,
            Some(self.assets_dir()),
            RenderOptions { scale: LINT_SCALE },
        )?;
        let mut report = LintReport::default();
        let mut failed: HashSet<&str> = HashSet::new();
        let mut checked: HashSet<&str> = HashSet::new();
        for (index, scene) in self.video.scenes.iter().enumerate() {
            let name = scene.component.as_str();
            if checked.insert(name) {
                let loaded = self
                    .components
                    .read(name)
                    .and_then(|tsx| engine.load_component(name, &tsx).map(|_| ()));
                match loaded {
                    Ok(()) => lint_examples(&engine, name, &mut report),
                    Err(error) => {
                        failed.insert(name);
                        report.issues.push(LintIssue {
                            scene: Some(index),
                            component: name.to_owned(),
                            example: None,
                            error,
                        });
                    }
                }
            }
            if failed.contains(name) {
                continue;
            }
            let end = scene.hold - (scene.hold / 2.0).min(1e-3);
            let mut seen: HashSet<String> = HashSet::new();
            for t in [0.0, end] {
                let call = SceneCall {
                    component: name,
                    props: &scene.props,
                    t,
                    duration: scene.hold,
                    index,
                };
                if let Err(error) = engine.render(call)
                    && seen.insert(error.to_string())
                {
                    report.issues.push(LintIssue {
                        scene: Some(index),
                        component: name.to_owned(),
                        example: None,
                        error,
                    });
                }
            }
        }
        Ok(report)
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
                component: name.to_owned(),
                example: Some(example.clone()),
                error,
            });
        }
    }
}

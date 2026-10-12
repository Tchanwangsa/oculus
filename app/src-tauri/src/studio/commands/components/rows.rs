//! The component list: the library cascade without a video, each component
//! loaded for its props schema and examples.

use std::sync::Arc;

use serde::Serialize;
use serde_json::{Map, Value};
use studio_core::{CascadeComponents, ComponentSource, FolderComponents, Library, PropSpec};

use super::super::levels::{roots, Level};
use super::engine;

/// Mirrors `ComponentInfo` in `app/src/lib/studio/components.ts`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ComponentRow {
    pub name: String,
    pub level: Level,
    pub kind: Kind,
    /// In declaration order; empty when the component does not load.
    pub props: Vec<NamedProp>,
    pub examples: Map<String, Value>,
    pub problem: Option<String>,
    /// A subject's file that replaces a global component of the same name:
    /// the subject's "own version".
    pub shadows: bool,
}

/// A shell lays out named areas (`SplitShell`); a component is any other
/// scene. Nothing in a component's code says which, so the name does: a
/// shell's ends in `Shell`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Shell,
    Component,
}

impl Kind {
    pub fn of(name: &str) -> Kind {
        if name.ends_with("Shell") {
            Kind::Shell
        } else {
            Kind::Component
        }
    }
}

/// One prop: its name beside the builder's own JSON (`kind`, `optional`,
/// `default`, and `of`, `values` or `token`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct NamedProp {
    pub name: String,
    #[serde(flatten)]
    pub spec: PropSpec,
}

/// Every component in `subject`'s library and the global one, one row per
/// name from the nearest level, sorted by name.
pub fn list_in(library: &Library, subject: Option<&str>) -> Result<Vec<ComponentRow>, String> {
    let roots = roots(library, subject)?;
    let global = roots
        .iter()
        .find(|(level, _)| *level == Level::Global)
        .map(|(_, root)| root.join("components"));
    let sources: Vec<(String, Arc<dyn ComponentSource>)> = roots
        .into_iter()
        .map(|(level, root)| {
            let source: Arc<dyn ComponentSource> =
                Arc::new(FolderComponents::new(root.join("components")));
            (level.name().to_owned(), source)
        })
        .collect();
    let cascade = CascadeComponents::new(sources);
    let mut loader = engine::loader()?;
    let mut rows = Vec::new();
    for (name, level) in cascade.levels() {
        let level = if level == Level::Subject.name() {
            Level::Subject
        } else {
            Level::Global
        };
        let shadows = level == Level::Subject
            && global
                .as_ref()
                .is_some_and(|dir| dir.join(format!("{name}.tsx")).is_file());
        let loaded = cascade
            .read(&name)
            .and_then(|tsx| loader.load_component(&name, &tsx).cloned())
            .map_err(|e| e.to_string());
        rows.push(match loaded {
            Ok(info) => ComponentRow {
                kind: Kind::of(&name),
                props: info
                    .props
                    .into_iter()
                    .map(|(name, spec)| NamedProp { name, spec })
                    .collect(),
                examples: info.examples,
                problem: None,
                shadows,
                name,
                level,
            },
            Err(problem) => ComponentRow {
                kind: Kind::of(&name),
                props: Vec::new(),
                examples: Map::new(),
                problem: Some(problem),
                shadows,
                name,
                level,
            },
        });
    }
    Ok(rows)
}

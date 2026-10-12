//! The two library levels the playground's commands reach: a subject's
//! (`<data>/studio/subjects/<CODE>/`) and the global one. A command names a
//! level and a subject code, never a path, so it cannot leave either library.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use studio_core::{valid_subject, Library};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Subject,
    Global,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Subject => "subject",
            Level::Global => "global",
        }
    }
}

/// The library folders a read reaches, nearest first: the subject's when one
/// is named, then the global one.
pub fn roots(library: &Library, subject: Option<&str>) -> Result<Vec<(Level, PathBuf)>, String> {
    let mut roots = Vec::new();
    if subject.is_some() {
        roots.push((Level::Subject, root_of(library, Level::Subject, subject)?));
    }
    roots.push((Level::Global, root_of(library, Level::Global, subject)?));
    Ok(roots)
}

/// The folder of one level. `subject` is checked whenever it is given, and
/// the subject level needs it.
pub fn root_of(library: &Library, level: Level, subject: Option<&str>) -> Result<PathBuf, String> {
    if let Some(code) = subject {
        if !valid_subject(code) {
            return Err(format!(
                "`{code}` is not a subject: a course code's folder name, in letters, digits, `-` and `_`"
            ));
        }
    }
    match level {
        Level::Global => library
            .global
            .clone()
            .ok_or_else(|| "there is no global Studio library".to_owned()),
        Level::Subject => {
            let code = subject.ok_or("a subject-level library needs a subject")?;
            let subjects = library
                .subjects
                .as_ref()
                .ok_or("there are no subject Studio libraries")?;
            Ok(subjects.join(code))
        }
    }
}

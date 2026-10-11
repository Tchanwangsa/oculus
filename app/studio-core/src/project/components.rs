//! Where a component name's TSX comes from: the project's `components/`
//! folder, and the cascade over the project, subject and global libraries
//! (`CascadeComponents`, first hit by name wins).

use crate::error::{Result, StudioError};
use crate::suggest::did_you_mean;
use std::path::PathBuf;
use std::sync::Arc;

pub trait ComponentSource: Send + Sync {
    /// The TSX source of the named component.
    fn read(&self, name: &str) -> Result<String>;
    /// Every component name this source can resolve, sorted.
    fn names(&self) -> Vec<String>;
}

/// `components/<Name>.tsx` in one folder.
pub struct FolderComponents {
    dir: PathBuf,
}

impl FolderComponents {
    pub fn new(dir: PathBuf) -> FolderComponents {
        FolderComponents { dir }
    }
}

/// Component names are PascalCase identifiers, which also keeps them
/// inside the folder.
pub(crate) fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}

impl ComponentSource for FolderComponents {
    fn read(&self, name: &str) -> Result<String> {
        if !valid_name(name) {
            return Err(StudioError::ComponentNotFound {
                name: name.to_owned(),
                message: "component names are PascalCase letters and digits, like TitleCard"
                    .to_owned(),
            });
        }
        let path = self.dir.join(format!("{name}.tsx"));
        match std::fs::read_to_string(&path) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let names = self.names();
                Err(StudioError::ComponentNotFound {
                    name: name.to_owned(),
                    message: format!(
                        "no file {}{}; available: {}",
                        path.display(),
                        did_you_mean(name, names.iter().map(String::as_str)),
                        if names.is_empty() {
                            "none".to_owned()
                        } else {
                            names.join(", ")
                        }
                    ),
                })
            }
            Err(e) => Err(StudioError::io(path, e)),
        }
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let stem = name.strip_suffix(".tsx")?;
                valid_name(stem).then(|| stem.to_owned())
            })
            .collect();
        names.sort();
        names
    }
}

/// Several sources in priority order: a name resolves to the first source
/// that has it, and `names` is the union. The project's folder comes first,
/// then the subject's library, then the global one.
pub struct CascadeComponents {
    sources: Vec<Arc<dyn ComponentSource>>,
}

impl CascadeComponents {
    pub fn new(sources: Vec<Arc<dyn ComponentSource>>) -> CascadeComponents {
        CascadeComponents { sources }
    }
}

impl ComponentSource for CascadeComponents {
    fn read(&self, name: &str) -> Result<String> {
        if !valid_name(name) {
            return Err(StudioError::ComponentNotFound {
                name: name.to_owned(),
                message: "component names are PascalCase letters and digits, like TitleCard"
                    .to_owned(),
            });
        }
        for source in &self.sources {
            if source.names().iter().any(|n| n == name) {
                return source.read(name);
            }
        }
        let names = self.names();
        Err(StudioError::ComponentNotFound {
            name: name.to_owned(),
            message: format!(
                "no file {name}.tsx in the project, subject or global library{}; available: {}",
                did_you_mean(name, names.iter().map(String::as_str)),
                if names.is_empty() {
                    "none".to_owned()
                } else {
                    names.join(", ")
                }
            ),
        })
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.sources.iter().flat_map(|s| s.names()).collect();
        names.sort();
        names.dedup();
        names
    }
}

#[cfg(test)]
mod tests;

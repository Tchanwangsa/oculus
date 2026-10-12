//! `promote`: a video's own component or theme copied into the global
//! library or its subject's, through `studio_core::write_library`, so the
//! library's previous text lands in `.history` first. The video keeps its
//! copy, which still wins the cascade for that video.

use std::path::{Component, Path, PathBuf};

use studio_core::{valid_theme_name, write_library, Library, Project, Theme};

/// Which library a promote writes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Global,
    /// The library of the subject the video's `video.json` names.
    Subject,
}

/// What a promote did.
#[derive(Debug, PartialEq)]
pub struct Promoted {
    /// The library file.
    pub path: PathBuf,
    /// `global` or `subject <CODE>`.
    pub level: String,
    /// False when the library already held the same text, so nothing was
    /// written.
    pub changed: bool,
}

/// Copies the video's `components/<name>.tsx` into the `target` library.
/// The component must be the video's own (level `project`), not one the
/// cascade finds in a library.
pub fn promote_component(
    dir: &Path,
    name: &str,
    target: Target,
    library: &Library,
) -> Result<Promoted, String> {
    let project = Project::load_unnarrated_with(dir, library).map_err(|e| e.to_string())?;
    match project.components.levels().iter().find(|(n, _)| n == name) {
        Some((_, level)) if level == "project" => {}
        Some((_, level)) => {
            return Err(format!(
                "`{name}` comes from the {level} library, not {}/components/; promote copies a video's own component",
                dir.display()
            ))
        }
        None => {
            return Err(format!(
                "no component `{name}` in {}/components/",
                dir.display()
            ))
        }
    }
    let source = dir.join("components").join(format!("{name}.tsx"));
    let text = std::fs::read_to_string(&source)
        .map_err(|e| format!("cannot read {}: {e}", source.display()))?;
    write(
        &project,
        library,
        target,
        &format!("components/{name}.tsx"),
        &text,
    )
}

/// Copies the theme file `file` (a `.json` path inside the video folder)
/// into the `target` library as `themes/<stem>.json`, after checking that it
/// loads as a theme. The video can then name it by its stem.
pub fn promote_theme(
    dir: &Path,
    file: &Path,
    target: Target,
    library: &Library,
) -> Result<Promoted, String> {
    let inside = file
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if !inside || file.extension().is_none_or(|e| e != "json") {
        return Err(format!(
            "`{}` must be a .json path inside the video folder, e.g. themes/chalk.json",
            file.display()
        ));
    }
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| valid_theme_name(s))
        .ok_or_else(|| {
            format!(
                "`{}`: a library theme's name is its file stem, in lowercase letters, digits and `-`",
                file.display()
            )
        })?;
    let source = dir.join(file);
    Theme::from_file(&source).map_err(|e| e.to_string())?;
    let text = std::fs::read_to_string(&source)
        .map_err(|e| format!("cannot read {}: {e}", source.display()))?;
    let project = Project::load_unnarrated_with(dir, library).map_err(|e| e.to_string())?;
    write(
        &project,
        library,
        target,
        &format!("themes/{stem}.json"),
        &text,
    )
}

/// Writes `text` at `rel` in the target library, unless it is there already.
fn write(
    project: &Project,
    library: &Library,
    target: Target,
    rel: &str,
    text: &str,
) -> Result<Promoted, String> {
    let (root, level) = match target {
        Target::Global => (
            library
                .global
                .clone()
                .ok_or("no global library to promote into")?,
            "global".to_owned(),
        ),
        Target::Subject => {
            let code = project.video.subject.as_deref().ok_or_else(|| {
                format!(
                    "{}/video.json names no subject; add \"subject\": \"<CODE>\" to it, or promote --to global",
                    project.dir.display()
                )
            })?;
            let subjects = library
                .subjects
                .as_ref()
                .ok_or("no subject libraries to promote into")?;
            (subjects.join(code), format!("subject {code}"))
        }
    };
    let path = root.join(rel);
    let current = match std::fs::read_to_string(&path) {
        Ok(current) => Some(current),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let changed = current.as_deref() != Some(text);
    if changed {
        write_library(&root, rel, text).map_err(|e| e.to_string())?;
    }
    Ok(Promoted {
        path,
        level,
        changed,
    })
}

//! `oculus studio brief`: a scene builder's whole input in one read, so it
//! never opens the whole storyboard or the narration manifest: the scene,
//! what its neighbours hand on and expect, its storyboard entry
//! (`storyboard`), the notes it names, its words at the times `word()`
//! returns, and the commands that check it (`text` prints it;
//! docs/studio.md, "`oculus studio` renders and lints from the terminal").

mod storyboard;
#[cfg(test)]
mod tests;
mod text;

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};
use studio_core::{ComponentSource, CuedWord, Project};

use super::videos::valid_slug;

#[derive(Debug, Serialize)]
pub struct Brief {
    pub index: usize,
    pub count: usize,
    pub id: Option<String>,
    pub component: String,
    pub props: Map<String, Value>,
    /// The video folder, absolute.
    pub dir: PathBuf,
    pub previous: Option<Neighbour>,
    pub next: Option<Neighbour>,
    /// The scene's storyboard entry, verbatim.
    pub storyboard: Option<String>,
    /// Why there is no entry, when there is none.
    pub storyboard_missing: Option<String>,
    pub notes: Notes,
    pub narration: Narration,
    /// The exact strip, frame and gallery calls for this scene.
    pub commands: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Neighbour {
    pub index: usize,
    pub id: Option<String>,
    pub component: String,
    /// Whether its component resolves yet (the folder or a library).
    pub written: bool,
    /// Whether the storyboard has an entry for it.
    pub entry: bool,
    pub carries_in: Option<String>,
    pub hands_on: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Notes {
    /// True when the paths are the ones the entry names; false when they
    /// are every file in `notes/`.
    pub named: bool,
    /// Relative to the video folder.
    pub paths: Vec<String>,
    /// Named paths with no file.
    pub missing: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Narration {
    /// A silent scene and its hold.
    Silent { seconds: f64 },
    /// The scene's length and its words, one list per sentence.
    Spoken {
        seconds: f64,
        sentences: Vec<Vec<CuedWord>>,
    },
    /// Narration missing or stale (or a silent scene without a hold): the
    /// error, and the script section's text when there is one.
    Unavailable {
        error: String,
        script: Option<String>,
    },
}

/// The scene `arg` names: an id from video.json, else a 0-based index.
pub fn scene_index(project: &Project, arg: &str) -> Result<usize, String> {
    let scenes = &project.video.scenes;
    if let Some(i) = scenes.iter().position(|s| s.id.as_deref() == Some(arg)) {
        return Ok(i);
    }
    let ids: Vec<&str> = scenes.iter().filter_map(|s| s.id.as_deref()).collect();
    match arg.parse::<usize>() {
        Ok(i) if i < scenes.len() => Ok(i),
        Ok(i) => Err(format!(
            "scene {i} is out of range: the video has {} scenes, 0 to {}",
            scenes.len(),
            scenes.len().saturating_sub(1)
        )),
        Err(_) => Err(format!(
            "no scene `{arg}`; the ids are {}",
            if ids.is_empty() {
                "none (use an index)".to_owned()
            } else {
                ids.join(", ")
            }
        )),
    }
}

/// How the printed commands name the video: its slug when it lives in
/// `videos`, else its absolute path, quoted for a shell when it needs it.
pub fn target(dir: &Path, videos: &Path) -> String {
    let slug = dir
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| valid_slug(s));
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_owned());
    match slug {
        Some(s) if dir.parent().map(canon) == Some(canon(videos)) => s.to_owned(),
        _ => shell_word(&dir.to_string_lossy()),
    }
}

/// Scene `index`'s brief. `target` names the video in the commands
/// (`target`); `dir` is the folder, absolute.
pub fn build(project: &Project, index: usize, dir: &Path, target: &str) -> Brief {
    let scenes = &project.video.scenes;
    let scene = &scenes[index];
    let source = std::fs::read_to_string(dir.join("storyboard.md")).ok();
    let entry_of = |i: usize| {
        let id = scenes[i].id.as_deref()?;
        storyboard::entry(source.as_deref()?, id)
    };
    let names = project.components.names();
    let neighbour = |i: usize| {
        let spec = scenes.get(i)?;
        let entry = entry_of(i);
        let (carries_in, hands_on) = entry
            .as_deref()
            .map(storyboard::handoff)
            .unwrap_or_default();
        Some(Neighbour {
            index: i,
            id: spec.id.clone(),
            component: spec.component.clone(),
            written: names.contains(&spec.component),
            entry: entry.is_some(),
            carries_in,
            hands_on,
        })
    };
    let entry = entry_of(index);
    let storyboard_missing = match (&entry, &source, &scene.id) {
        (Some(_), _, _) => None,
        (None, None, _) => Some("no storyboard.md in the folder".to_owned()),
        (None, Some(_), None) => Some("the scene has no id to find its entry by".to_owned()),
        (None, Some(_), Some(id)) => Some(format!("storyboard.md has no `### {id}` entry")),
    };
    let label = scene.id.clone().unwrap_or_else(|| format!("scene{index}"));
    let out = dir.join("out");
    let png = |what: &str| shell_word(&out.join(format!("{label}-{what}.png")).to_string_lossy());
    Brief {
        index,
        count: scenes.len(),
        id: scene.id.clone(),
        component: scene.component.clone(),
        props: scene.props.clone(),
        dir: dir.to_owned(),
        previous: index.checked_sub(1).and_then(neighbour),
        next: neighbour(index + 1),
        notes: notes(dir, entry.as_deref()),
        storyboard: entry,
        storyboard_missing,
        narration: narration(project, index),
        commands: vec![
            format!(
                "oculus studio strip {target} --scene {index} --n 16 --cols 4 -o {}",
                png("strip")
            ),
            format!(
                "oculus studio frame {target} --scene {index} --at <S> --scale 0.5 -o {}",
                png("frame")
            ),
            format!("oculus studio gallery {target} {} --sheet", scene.component),
        ],
    }
}

/// The notes the entry names, else every file in `notes/`.
fn notes(dir: &Path, entry: Option<&str>) -> Notes {
    let named = entry.map(storyboard::note_paths).unwrap_or_default();
    if !named.is_empty() {
        let missing = named
            .iter()
            .filter(|p| !dir.join(p).is_file())
            .cloned()
            .collect();
        return Notes {
            named: true,
            paths: named,
            missing,
        };
    }
    let mut paths: Vec<String> = std::fs::read_dir(dir.join("notes"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().to_str().map(|n| format!("notes/{n}")))
        .collect();
    paths.sort();
    Notes {
        named: false,
        paths,
        missing: Vec::new(),
    }
}

fn narration(project: &Project, index: usize) -> Narration {
    let unavailable = |e: studio_core::StudioError| Narration::Unavailable {
        error: e.to_string(),
        script: project.section(index).map(|s| s.text.clone()),
    };
    let seconds = match project.scene_length(index) {
        Ok(s) => s,
        Err(e) => return unavailable(e),
    };
    match project.cued_words(index) {
        Ok(Some(words)) => Narration::Spoken {
            seconds,
            sentences: sentences(words),
        },
        Ok(None) => Narration::Silent { seconds },
        Err(e) => unavailable(e),
    }
}

/// `words` split after each word that ends a sentence (`.`, `?`, `!`,
/// before any closing quote or bracket).
fn sentences(words: Vec<CuedWord>) -> Vec<Vec<CuedWord>> {
    let mut out: Vec<Vec<CuedWord>> = Vec::new();
    let mut current = Vec::new();
    for w in words {
        let ends = w
            .text
            .trim_end_matches(['"', '\'', '\u{201d}', '\u{2019}', ')', ']'])
            .ends_with(['.', '?', '!']);
        current.push(w);
        if ends {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// `s` as one shell word: bare when it is only safe characters, else in
/// single quotes.
fn shell_word(s: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "_./-:@%+=,".contains(c);
    if !s.is_empty() && s.chars().all(safe) {
        return s.to_owned();
    }
    format!("'{}'", s.replace('\'', r"'\''"))
}

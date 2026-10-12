//! `assets/slides/<name>.json`, written beside each crop: where it came from
//! (the PDF, its sha256, the page and box), how it was chosen (to re-run
//! it), the blocks inside it as fractions of the crop, and the `Slide`
//! props to paste. Lint warns when the PDF has changed since.

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use studio_core::{LintIssue, LintLevel, Project, StudioError};

use super::geometry::{fmt_box, Frac};
use super::snap::Edges;

/// The folder under a video's `assets/` that slides go in.
pub const SLIDES_DIR: &str = "slides";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sidecar {
    /// The PDF, relative to the data directory.
    pub source: String,
    /// Of the PDF's bytes when the crop was made.
    pub sha256: String,
    /// 1-based.
    pub page: u32,
    /// The page's size in points as rendered (crop box, rotated).
    pub page_size: [f64; 2],
    /// The crop as fractions of the page, padding included.
    pub bbox: Frac,
    pub select: Select,
    pub pad: f64,
    /// `--exact`: the padded box as it is, no edge moved off ink.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exact: bool,
    /// What each edge did to stay off ink (`snap`); none with `exact`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edges: Option<Edges>,
    /// The `--width` asked for, if one was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    pub image: ImageRef,
    pub page_image: ImageRef,
    /// Blocks at least half inside the crop.
    pub blocks: Vec<Inner>,
    /// The `Slide` starter's props for this crop.
    pub props: serde_json::Value,
}

/// How the region was chosen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Select {
    Find(String),
    Blocks(Vec<usize>),
    Bbox(Frac),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageRef {
    /// Relative to `assets/`, as `<Image src>` takes it.
    pub src: String,
    pub width: u32,
    pub height: u32,
}

/// A block inside the crop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inner {
    pub index: usize,
    pub kind: String,
    /// Fractions of the crop, clipped to it.
    pub bbox: Frac,
    pub text: String,
}

impl Sidecar {
    /// The `oculus studio slide` call that makes this crop again.
    pub fn command(&self, dir: &Path, name: &str) -> String {
        let how = match &self.select {
            Select::Find(text) => format!("--find {}", quote(text)),
            Select::Blocks(b) => format!(
                "--block {}",
                b.iter().map(usize::to_string).collect::<Vec<_>>().join(",")
            ),
            Select::Bbox(b) => format!("--bbox {}", fmt_box(b)),
        };
        let width = self
            .width
            .map_or(String::new(), |w| format!(" --width {w}"));
        let exact = if self.exact { " --exact" } else { "" };
        format!(
            "oculus studio slide {} {} --page {} {how} --pad {} --name {name}{width}{exact}",
            quote(&dir.to_string_lossy()),
            quote(&self.source),
            self.page,
            self.pad,
        )
    }
}

/// Hex sha256 of `bytes`.
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./,".contains(c))
    {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

/// A warning for each slide under `assets/slides/` whose PDF in `data_dir`
/// no longer has the sha256 it was cropped from. A PDF that cannot be read
/// (moved, another machine's library) is not checked; nothing is rendered.
pub fn lint(project: &Project, data_dir: &Path) -> Vec<LintIssue> {
    let folder = project.dir.join("assets").join(SLIDES_DIR);
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter_map(|n| n.strip_suffix(".json").map(str::to_owned))
        .collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let Some(sidecar) = std::fs::read_to_string(folder.join(format!("{name}.json")))
            .ok()
            .and_then(|text| serde_json::from_str::<Sidecar>(&text).ok())
        else {
            continue;
        };
        let rel = Path::new(&sidecar.source);
        if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
            continue;
        }
        let Ok(bytes) = std::fs::read(data_dir.join(rel)) else {
            continue;
        };
        if sha256(&bytes) == sidecar.sha256 {
            continue;
        }
        let scene = project.video.scenes.iter().position(|s| {
            serde_json::to_string(&s.props).is_ok_and(|p| p.contains(&sidecar.image.src))
        });
        let spec = scene.map(|i| &project.video.scenes[i]);
        out.push(LintIssue {
            level: LintLevel::Warning,
            scene,
            id: spec.and_then(|s| s.id.clone()),
            component: spec.map(|s| s.component.clone()),
            example: None,
            group: Some(("stale slide", name.clone())),
            error: StudioError::Lint(format!(
                "slide `{}` was cropped from {} page {}, which has changed since; \
                 check the crop still shows what the scene says and re-run `{}`",
                sidecar.image.src,
                sidecar.source,
                sidecar.page,
                sidecar.command(&project.dir, &name)
            )),
        });
    }
    out
}

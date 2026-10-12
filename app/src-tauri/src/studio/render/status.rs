//! Each preset's export as the Export menu shows it: the file, if one is
//! there, and whether it is current, meaning a render now would make the
//! same file. That is planned without rendering: the same segment keys and
//! track a render would record in renders.json.

use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;
use studio_core::{Library, Project};

use super::cache::{self, Used};
use super::error::RenderError;
use super::preset::Preset;
use super::toolkit::Toolkit;
use super::{audio, plan_video};

/// Mirrors `ExportStatus` in `app/src/lib/studio/render.ts`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportStatus {
    pub preset: Preset,
    /// The file at `Preset::default_out`, when there is one.
    pub path: Option<String>,
    /// Its mtime, in Unix seconds.
    pub modified: Option<u64>,
    /// The file is there and a render now would make the same file.
    pub current: bool,
    /// Why this preset cannot render now (stale narration, no encoder, a
    /// size too large).
    pub problem: Option<String>,
}

/// Every preset's status for the video in `dir`.
pub fn exports(dir: &Path, ffmpeg: &Path, library: &Library) -> Vec<ExportStatus> {
    let dir = &std::path::absolute(dir).unwrap_or_else(|_| dir.to_owned());
    let records = cache::records(dir);
    let project = Project::load_with(dir, library).map_err(RenderError::from);
    let toolkit = Toolkit::of(ffmpeg);
    Preset::ALL
        .into_iter()
        .map(|preset| {
            let out = preset.default_out(dir);
            let modified = out
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            let planned = match (&project, &toolkit) {
                (Ok(project), Ok(toolkit)) => planned(dir, project, preset, toolkit),
                (Err(e), _) | (_, Err(e)) => Err(e.to_string()),
            };
            let current = modified.is_some()
                && planned
                    .as_ref()
                    .is_ok_and(|p| records.get(preset.name()) == Some(p));
            ExportStatus {
                preset,
                path: modified.map(|_| out.to_string_lossy().into_owned()),
                modified,
                current,
                problem: planned.err(),
            }
        })
        .collect()
}

/// The record a render of `preset` to its default file would write now.
fn planned(
    dir: &Path,
    project: &Project,
    preset: Preset,
    toolkit: &Toolkit,
) -> Result<Used, String> {
    let fail = |e: RenderError| e.to_string();
    let plan = plan_video(dir, project, preset, toolkit).map_err(fail)?;
    let track = (!plan.clips.is_empty())
        .then(|| {
            let samples = audio::length(plan.frames, project.fps());
            cache::file_name(&cache::track(dir, &audio::key(&plan.clips, samples)))
        })
        .flatten();
    Ok(Used {
        segments: plan
            .segments
            .iter()
            .map(|s| format!("{}.mp4", s.key))
            .collect(),
        track,
        out: Some(preset.default_out(dir).to_string_lossy().into_owned()),
    })
}

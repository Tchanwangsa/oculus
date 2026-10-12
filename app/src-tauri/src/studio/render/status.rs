//! Each preset's export as the Export menu shows it: the file, if one is
//! there, and whether it is current, meaning a render now would make the
//! same file. That is planned without rendering: the same segment keys,
//! track and captions a render would record in renders.json.

use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;
use studio_core::{captions, Library, Project};

use super::cache::{self, Used};
use super::error::RenderError;
use super::export::cues_hash;
use super::preset::{Output, Preset};
use super::toolkit::Toolkit;
use super::{audio, plan, plan_video, scenes};

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
    /// Why this preset cannot render now (stale narration, no encoder, no
    /// narration to export, a size too large).
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
    let out = Some(preset.default_out(dir).to_string_lossy().into_owned());
    let fail = |e: RenderError| e.to_string();
    let track = |clips: &[audio::Clip], frames: u64| {
        let samples = audio::length(frames, project.fps());
        cache::file_name(&cache::track(dir, &audio::key(clips, samples)))
    };
    match preset.output() {
        Output::Video(_) => {
            let plan = plan_video(dir, project, preset, toolkit).map_err(fail)?;
            Ok(Used {
                segments: plan
                    .segments
                    .iter()
                    .map(|s| format!("{}.mp4", s.key))
                    .collect(),
                track: (!plan.clips.is_empty())
                    .then(|| track(&plan.clips, plan.frames))
                    .flatten(),
                out,
                cues: None,
            })
        }
        Output::Audio => {
            let scenes = scenes(project).map_err(|e| e.to_string())?;
            let clips = plan::clips(&scenes, project.fps(), project.theme.narration.lead);
            if clips.is_empty() {
                return Err("the video has no narration".to_owned());
            }
            toolkit.check_audio().map_err(fail)?;
            let frames = scenes.last().map_or(0, |s| s.frames.end);
            Ok(Used {
                track: track(&clips, frames),
                out,
                ..Used::default()
            })
        }
        Output::Subtitles => {
            let cues = project.caption_cues().map_err(|e| e.to_string())?;
            if cues.is_empty() {
                return Err("the video has no narration".to_owned());
            }
            Ok(Used {
                out,
                cues: Some(cues_hash(&captions::vtt(&cues))),
                ..Used::default()
            })
        }
    }
}

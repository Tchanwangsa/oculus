//! The `.vtt` a narrated video render writes beside its mp4.

use std::path::{Path, PathBuf};

use studio_core::{captions, Project};

use super::encoder::part_path;
use super::error::RenderError;

/// The captions as WebVTT beside a video render's `out` (`.vtt` for its
/// extension), whether or not the video burns them in; `None` when the
/// video has no narration. No ffmpeg.
pub(super) fn captions_beside(
    project: &Project,
    out: &Path,
) -> Result<Option<PathBuf>, RenderError> {
    let cues = project.caption_cues()?;
    if cues.is_empty() {
        return Ok(None);
    }
    let path = out.with_extension("vtt");
    let part = part_path(&path);
    std::fs::write(&part, captions::vtt(&cues)).map_err(|e| RenderError::io(&part, e))?;
    std::fs::rename(&part, &path).map_err(|e| RenderError::io(&path, e))?;
    Ok(Some(path))
}

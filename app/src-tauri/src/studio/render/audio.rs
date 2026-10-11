//! The narration track, mixed in Rust rather than by ffmpeg's `amix`, which
//! divides by its input count and changes levels: each scene's clip is
//! decoded, resampled to 48 kHz mono and placed at its scene's start plus
//! the theme's `lead`, in one buffer as long as the video. Clips never
//! overlap (a scene outlasts lead + clip + tail), so placing is adding to
//! silence. Written as 16-bit PCM WAV, which ffmpeg encodes to AAC.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::cache;
use super::error::RenderError;
use crate::studio::audio::{decode_wav, resample};

/// The track's sample rate.
pub(super) const RATE: u32 = 48_000;

/// Bumped when the mix changes in a way its inputs do not show.
const SCHEME: &str = "oculus-render-audio-1";

/// One clip and where it starts in the video.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Clip {
    pub path: PathBuf,
    /// Global seconds.
    pub at: f64,
}

/// The sample a clip starting at `at` seconds lands on.
pub(super) fn offset(at: f64) -> usize {
    (at * f64::from(RATE)).round().max(0.0) as usize
}

/// The track's length in samples for a video of `frames` frames at `fps`.
pub(super) fn length(frames: u64, fps: u32) -> usize {
    (frames as f64 * f64::from(RATE) / f64::from(fps)).round() as usize
}

/// sha256 hex of the clips' names and offsets and the track's length: the
/// clips are content-addressed (`narration/<key>.wav`), so this names the
/// mix.
pub(super) fn key(clips: &[Clip], samples: usize) -> String {
    let mut hash = Sha256::new();
    let mut part = |s: &str| {
        hash.update((s.len() as u64).to_le_bytes());
        hash.update(s.as_bytes());
    };
    part(SCHEME);
    part(&samples.to_string());
    for c in clips {
        let name = c.path.file_name().map(|n| n.to_string_lossy().into_owned());
        part(&name.unwrap_or_default());
        part(&offset(c.at).to_string());
    }
    format!("{:x}", hash.finalize())
}

/// The mixed track, `samples` long. A clip that would run past the end is
/// cut there.
pub(super) fn mix(clips: &[Clip], samples: usize) -> Result<Vec<f32>, RenderError> {
    let mut track = vec![0.0f32; samples];
    for clip in clips {
        let bytes = std::fs::read(&clip.path).map_err(|e| RenderError::io(&clip.path, e))?;
        let decoded = decode_wav(&bytes).map_err(|e| {
            RenderError::Render(format!("narration clip {}: {e}", clip.path.display()))
        })?;
        let pcm = resample(&decoded.samples, decoded.sample_rate, RATE).map_err(|e| {
            RenderError::Render(format!("narration clip {}: {e}", clip.path.display()))
        })?;
        place(&mut track, &pcm, offset(clip.at));
    }
    Ok(track)
}

/// Add `pcm` into `track` from sample `at`.
pub(super) fn place(track: &mut [f32], pcm: &[f32], at: usize) {
    if at >= track.len() {
        return;
    }
    for (out, s) in track[at..].iter_mut().zip(pcm) {
        *out += s;
    }
}

/// Write `track` as 16-bit mono WAV at `RATE`, through a `.part` file.
pub(super) fn write(path: &Path, track: &[f32]) -> Result<(), RenderError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let part = super::encoder::part_path(path);
    let fail = |e: hound::Error| RenderError::io(&part, e);
    let mut writer = hound::WavWriter::create(&part, spec).map_err(fail)?;
    for s in track {
        let v = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        writer.write_sample(v).map_err(fail)?;
    }
    writer.finalize().map_err(fail)?;
    std::fs::rename(&part, path).map_err(|e| RenderError::io(path, e))
}

/// The mixed track for `clips`, from the cache or made now; `None` for a
/// video with no narration.
pub(super) fn track(
    dir: &Path,
    clips: &[Clip],
    samples: usize,
) -> Result<Option<PathBuf>, RenderError> {
    if clips.is_empty() {
        return Ok(None);
    }
    let path = cache::track(dir, &key(clips, samples));
    if !path.is_file() {
        write(&path, &mix(clips, samples)?)?;
    }
    Ok(Some(path))
}

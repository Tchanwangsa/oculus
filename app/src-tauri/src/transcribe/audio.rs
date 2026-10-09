//! The audio an engine hears: extracted from the recording with ffmpeg, and
//! cut by time when it is larger than one upload may be.

use std::path::Path;
use std::process::Command;

/// Ogg Opus. Speech needs nothing past 16 kHz mono (Whisper resamples to it),
/// and Opus at 24 kbit/s keeps an hour near 11 MB — one upload for most
/// recordings. Constrained VBR keeps size proportional to time, which the
/// chunk plan assumes. The bundled ffmpeg builds `libopus`.
pub(super) const EXTENSION: &str = ".ogg";
pub(super) const CONTENT_TYPE: &str = "audio/ogg";

pub(super) struct Extracted {
    pub bytes: u64,
    /// The recording's length as ffmpeg reported it; needed only to chunk.
    pub duration: Option<f64>,
}

/// A stretch of the extracted audio. `len: None` runs to the end, so rounding
/// never drops the tail.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Span {
    pub start: f64,
    pub len: Option<f64>,
}

/// Decode `video`'s first audio stream into `out`.
pub(super) fn extract(ffmpeg: &Path, video: &Path, out: &Path) -> Result<Extracted, String> {
    let output = Command::new(ffmpeg)
        .args(["-nostdin", "-hide_banner", "-nostats", "-y", "-i"])
        .arg(video)
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-c:a",
            "libopus",
            "-b:a",
            "24k",
            "-vbr",
            "constrained",
            "-application",
            "voip",
        ])
        .arg(out)
        .output()
        .map_err(|e| format!("could not run ffmpeg: {e}"))?;
    let log = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        if log.contains("matches no streams") {
            return Err("this recording has no audio track to transcribe".into());
        }
        return Err(format!(
            "ffmpeg could not extract the audio: {}",
            last_line(&log)
        ));
    }
    let bytes = std::fs::metadata(out).map_err(|e| e.to_string())?.len();
    Ok(Extracted {
        bytes,
        duration: duration(&log),
    })
}

/// Decode `audio` to 16 kHz mono 16-bit PCM WAV, for an engine that cannot
/// read Ogg Opus (whisper-cli). About 115 MB an hour, so only in the scratch dir.
pub(super) fn to_wav(ffmpeg: &Path, audio: &Path, out: &Path) -> Result<(), String> {
    let output = Command::new(ffmpeg)
        .args(["-nostdin", "-hide_banner", "-nostats", "-y", "-i"])
        .arg(audio)
        .args(["-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"])
        .arg(out)
        .output()
        .map_err(|e| format!("could not run ffmpeg: {e}"))?;
    if !output.status.success() {
        let log = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffmpeg could not decode the audio to WAV: {}",
            last_line(&log)
        ));
    }
    Ok(())
}

/// Copy one span of `audio` into `out`, without re-encoding.
pub(super) fn cut(ffmpeg: &Path, audio: &Path, out: &Path, span: &Span) -> Result<(), String> {
    let mut command = Command::new(ffmpeg);
    command
        .args(["-nostdin", "-hide_banner", "-nostats", "-y", "-ss"])
        .arg(format!("{:.3}", span.start))
        .arg("-i")
        .arg(audio);
    if let Some(len) = span.len {
        command.arg("-t").arg(format!("{len:.3}"));
    }
    let output = command
        .args(["-c", "copy"])
        .arg(out)
        .output()
        .map_err(|e| format!("could not run ffmpeg: {e}"))?;
    if !output.status.success() {
        let log = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffmpeg could not split the audio: {}",
            last_line(&log)
        ));
    }
    Ok(())
}

/// Split `bytes` of audio lasting `duration` into the fewest equal-length
/// spans that each fit `cap`. Cuts fall at fixed times, with no overlap.
pub(super) fn plan(
    bytes: u64,
    duration: Option<f64>,
    cap: Option<u64>,
) -> Result<Vec<Span>, String> {
    let whole = vec![Span {
        start: 0.0,
        len: None,
    }];
    let Some(cap) = cap.filter(|&cap| cap > 0 && bytes > cap) else {
        return Ok(whole);
    };
    let duration = duration
        .filter(|d| d.is_finite() && *d > 0.0)
        .ok_or("the audio is too large for one upload and ffmpeg did not report its length")?;
    let parts = bytes.div_ceil(cap) as usize;
    let len = duration / parts as f64;
    Ok((0..parts)
        .map(|i| Span {
            start: len * i as f64,
            len: (i + 1 < parts).then_some(len),
        })
        .collect())
}

/// The input's `Duration: HH:MM:SS.ss` from ffmpeg's log.
pub(crate) fn duration(log: &str) -> Option<f64> {
    let rest = log.split("Duration: ").nth(1)?;
    let stamp = rest.split(',').next()?.trim();
    let mut parts = stamp.split(':');
    let (h, m, s) = (parts.next()?, parts.next()?, parts.next()?);
    let seconds =
        h.parse::<f64>().ok()? * 3600.0 + m.parse::<f64>().ok()? * 60.0 + s.parse::<f64>().ok()?;
    Some(seconds)
}

fn last_line(log: &str) -> &str {
    log.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("no output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_under_the_cap_is_one_whole_span() {
        let whole = vec![Span {
            start: 0.0,
            len: None,
        }];
        assert_eq!(
            plan(19_000_000, Some(5000.0), Some(20_000_000)).unwrap(),
            whole
        );
        assert_eq!(plan(20_000_000, None, Some(20_000_000)).unwrap(), whole);
        // An engine with no cap reads any length, even without a duration.
        assert_eq!(plan(900_000_000, None, None).unwrap(), whole);
    }

    #[test]
    fn audio_over_the_cap_splits_into_equal_spans_ending_open() {
        let spans = plan(45_000_000, Some(9000.0), Some(20_000_000)).unwrap();
        assert_eq!(
            spans,
            vec![
                Span {
                    start: 0.0,
                    len: Some(3000.0)
                },
                Span {
                    start: 3000.0,
                    len: Some(3000.0)
                },
                Span {
                    start: 6000.0,
                    len: None
                },
            ]
        );
    }

    #[test]
    fn just_over_the_cap_is_two_spans() {
        let spans = plan(20_000_001, Some(7000.0), Some(20_000_000)).unwrap();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].start, 3500.0);
    }

    #[test]
    fn chunking_without_a_duration_is_an_error_not_a_guess() {
        assert!(plan(45_000_000, None, Some(20_000_000)).is_err());
        assert!(plan(45_000_000, Some(0.0), Some(20_000_000)).is_err());
    }

    #[test]
    fn the_input_duration_is_read_from_ffmpegs_log() {
        let log = "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'source1.mp4':\n  \
                   Duration: 01:52:07.48, start: 0.000000, bitrate: 412 kb/s\n";
        assert_eq!(duration(log), Some(6727.48));
        assert_eq!(duration("  Duration: N/A, bitrate: N/A"), None);
        assert_eq!(duration("nothing here"), None);
    }

    #[test]
    fn the_last_log_line_names_the_failure() {
        assert_eq!(
            last_line("a\nStream map '0:a:0' matches no streams.\n\n"),
            "Stream map '0:a:0' matches no streams."
        );
        assert_eq!(last_line(""), "no output");
    }
}

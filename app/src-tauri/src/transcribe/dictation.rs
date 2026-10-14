//! Microphone clips use the same engine order and language as library videos.
//! Audio is kept in the run's scratch directory and removed on every exit.

use tauri::{AppHandle, Manager};

use super::{audio, pipeline, settings};

const MAX_CLIP_BYTES: usize = 24 * 1024 * 1024;

/// A read-only readiness check before opening the microphone. No inference.
#[tauri::command]
pub async fn dictation_status(app: AppHandle) -> Result<(), String> {
    let resource_dir = app.path().resource_dir().ok();
    crate::runtime::blocking::run(move || {
        let ffmpeg = crate::sources::echo360::find_ffmpeg(resource_dir.clone())
            .ok_or("Audio conversion is unavailable — install ffmpeg for Oculus.")?;
        settings::engines(resource_dir, &ffmpeg, None).map(|_| ())
    })
    .await
}

/// Decode a bounded microphone clip, returning prose rather than a saved VTT.
#[tauri::command]
pub async fn dictation_transcribe(app: AppHandle, audio: Vec<u8>) -> Result<String, String> {
    validate_clip(&audio)?;
    let resource_dir = app.path().resource_dir().ok();
    crate::runtime::blocking::run(move || {
        let ffmpeg = crate::sources::echo360::find_ffmpeg(resource_dir.clone())
            .ok_or("Audio conversion is unavailable — install ffmpeg for Oculus.")?;
        let engines = settings::engines(resource_dir, &ffmpeg, None)?;
        let scratch = pipeline::Scratch::new()?;
        let clip = scratch.file("microphone.audio");
        std::fs::write(&clip, audio)
            .map_err(|e| format!("Could not save the microphone clip: {e}"))?;
        let decoded = scratch.file(&format!("audio{}", audio::EXTENSION));
        let extracted = audio::extract(&ffmpeg, &clip, &decoded)?;
        let heard = pipeline::transcribe_audio(
            &engines,
            &extracted,
            &decoded,
            |engine, index, span| {
                let part = scratch.file(&format!("part-{engine}-{index}{}", audio::EXTENSION));
                audio::cut(&ffmpeg, &decoded, &part, span).map(|()| part)
            },
            &|_| {},
        )?;
        transcript(heard.segments)
    })
    .await
}

fn validate_clip(audio: &[u8]) -> Result<(), String> {
    if audio.is_empty() {
        return Err("No audio was recorded. Try dictating again.".into());
    }
    if audio.len() > MAX_CLIP_BYTES {
        return Err("This recording is too large. Dictate a shorter message.".into());
    }
    Ok(())
}

fn transcript(segments: Vec<super::Segment>) -> Result<String, String> {
    let text = segments
        .iter()
        .map(|s| s.text.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        return Err("No speech was heard. Check your microphone and try again.".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_oversized_clips_are_rejected_before_decoding() {
        assert!(validate_clip(&[]).is_err());
        assert!(validate_clip(&vec![0; MAX_CLIP_BYTES + 1]).is_err());
        assert!(validate_clip(&[1]).is_ok());
    }

    #[test]
    fn speech_is_joined_without_timestamps_or_empty_cues() {
        let segment = |text: &str| super::super::Segment {
            start: 0.0,
            end: 1.0,
            text: text.into(),
        };
        assert_eq!(
            transcript(vec![segment(" Hello "), segment(" "), segment("world.")]).unwrap(),
            "Hello world."
        );
        assert!(transcript(vec![segment(" ")]).is_err());
    }
}

//! whisper.cpp on this machine, through its `whisper-cli` helper: free,
//! offline once a model is downloaded (`whisper_models/`), on the GPU
//! through Metal on Apple Silicon, any length in one call.
//!
//! whisper-cli cannot read the pipeline's Ogg Opus, so each call first decodes
//! it to 16-bit PCM WAV beside it in the scratch dir. Silero VAD skips the
//! silence most recordings open with, where Whisper otherwise invents
//! "Thank you." loops.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::whisper_models;
use crate::transcribe::{audio, Engine, EngineError, Segment, Settings};

const HELPER: &str = "whisper-cli";

pub(in crate::transcribe) struct Whisper {
    helper: PathBuf,
    ffmpeg: PathBuf,
    model: PathBuf,
    /// Absent only when its download was lost; the run goes on without it.
    vad: Option<PathBuf>,
    /// A Whisper language code, or `auto`.
    language: String,
}

impl Whisper {
    /// The engine as Settings has it, or why it cannot run here.
    pub(in crate::transcribe) fn configured(
        settings: &Settings,
        resource_dir: Option<PathBuf>,
        ffmpeg: &Path,
    ) -> Result<Self, String> {
        if !settings.whisper {
            return Err("local Whisper is turned off in Settings → Transcription".into());
        }
        let dir = whisper_models::dir();
        let model = whisper_models::pick(&dir, settings.whisper_model.as_deref())?;
        let helper = helper(resource_dir)
            .ok_or("the Whisper helper is missing — run `bun run whisper` in app/")?;
        let vad = Some(dir.join(whisper_models::VAD_FILE)).filter(|p| p.is_file());
        Ok(Self {
            helper,
            ffmpeg: ffmpeg.to_path_buf(),
            model,
            vad,
            language: settings.language.whisper(),
        })
    }

    fn command(&self, wav: &Path, out: &Path) -> Command {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
        let mut command = Command::new(&self.helper);
        command
            .arg("-m")
            .arg(&self.model)
            .arg("-f")
            .arg(wav)
            .arg("-l")
            .arg(&self.language)
            .arg("-t")
            .arg(threads.to_string())
            .args(["-oj", "-np", "-sns"])
            .arg("-of")
            .arg(out);
        if let Some(vad) = &self.vad {
            command.arg("--vad").arg("-vm").arg(vad);
        }
        command
    }
}

/// `whisper-cli` beside the app, or dev's copy in `binaries/`.
pub(in crate::transcribe) fn helper(resource_dir: Option<PathBuf>) -> Option<PathBuf> {
    crate::runtime::bundled::find(HELPER, resource_dir)
}

impl Engine for Whisper {
    fn name(&self) -> &'static str {
        "whisper"
    }

    /// whisper-cli windows the audio itself.
    fn max_upload_bytes(&self) -> Option<u64> {
        None
    }

    fn transcribe(&self, audio: &Path) -> Result<Vec<Segment>, EngineError> {
        let wav = audio.with_extension("wav");
        audio::to_wav(&self.ffmpeg, audio, &wav).map_err(EngineError::Failed)?;
        // whisper-cli appends `.json` to `-of`.
        let out = audio.with_extension("whisper");
        let json = audio.with_extension("whisper.json");
        // No timeout: a long recording on a large model takes minutes.
        let output = self.command(&wav, &out).output();
        std::fs::remove_file(&wav).ok();
        let output = output
            .map_err(|e| EngineError::Failed(format!("could not run the Whisper helper: {e}")))?;
        let reply = std::fs::read(&json)
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        std::fs::remove_file(&json).ok();
        answer(
            output.status.code(),
            reply.as_deref(),
            &String::from_utf8_lossy(&output.stderr),
        )
    }
}

/// whisper-cli's exit, read. It exits 0 without writing the JSON when it
/// cannot read the audio or knows no such language, so a missing reply is a
/// failure too, named by its last `error` line.
fn answer(
    code: Option<i32>,
    reply: Option<&str>,
    stderr: &str,
) -> Result<Vec<Segment>, EngineError> {
    let lines = || {
        stderr
            .lines()
            .rev()
            .map(str::trim)
            .filter(|l| !l.is_empty())
    };
    let said = lines()
        .find(|l| l.to_lowercase().contains("error"))
        .or_else(|| lines().next());
    match (code, reply) {
        (Some(0), Some(reply)) => parse_reply(reply),
        (Some(0), None) => Err(EngineError::Failed(
            said.unwrap_or("the Whisper helper wrote no transcript")
                .to_string(),
        )),
        (Some(code), _) => Err(EngineError::Failed(match said {
            Some(line) => line.to_string(),
            None => format!("the Whisper helper exited with code {code}"),
        })),
        (None, _) => Err(EngineError::Failed("the Whisper helper was killed".into())),
    }
}

/// `-oj`'s `transcription[]`: `offsets.{from,to}` in milliseconds and the
/// segment's text. whisper-cli escapes only `"` and `\`, so other control
/// characters are blanked first; JSON's own layout needs none of them but `\n`.
pub(in crate::transcribe) fn parse_reply(body: &str) -> Result<Vec<Segment>, EngineError> {
    #[derive(serde::Deserialize)]
    struct Offsets {
        from: i64,
        to: i64,
    }
    #[derive(serde::Deserialize)]
    struct Item {
        offsets: Offsets,
        #[serde(default)]
        text: String,
    }
    #[derive(serde::Deserialize)]
    struct Reply {
        #[serde(default)]
        transcription: Vec<Item>,
    }
    let clean = body.replace(|c: char| c.is_control() && c != '\n', " ");
    let reply = serde_json::from_str::<Reply>(&clean)
        .map_err(|e| EngineError::Failed(format!("Whisper's reply was not a transcript: {e}")))?;
    Ok(reply
        .transcription
        .into_iter()
        .map(|item| Segment {
            start: item.offsets.from as f64 / 1000.0,
            end: item.offsets.to as f64 / 1000.0,
            text: item.text.trim().to_string(),
        })
        .filter(|s| !non_speech(&s.text))
        .collect())
}

/// Empty, or only markers such as `[BLANK_AUDIO]`, `[Music]`, `(silence)`,
/// `*laughs*` or `♪`, which caption nothing anyone said.
fn non_speech(text: &str) -> bool {
    let mut depth = 0i32;
    let mut starred = false;
    for c in text.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            '*' => starred = !starred,
            '♪' | '♫' => {}
            c if c.is_whitespace() || c.is_ascii_punctuation() => {}
            _ if depth > 0 || starred => {}
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_json_reply_becomes_segments_in_seconds_without_markers() {
        let segments = parse_reply(
            "{\"systeminfo\": \"AVX = 0\", \"result\": {\"language\": \"en\"}, \"transcription\": [\n\
             {\"timestamps\": {\"from\": \"00:00:00,000\", \"to\": \"00:00:30,000\"},\n\
              \"offsets\": {\"from\": 0, \"to\": 30000}, \"text\": \" [BLANK_AUDIO]\"},\n\
             {\"offsets\": {\"from\": 30000, \"to\": 33250}, \"text\": \" Good morning, everyone.\"},\n\
             {\"offsets\": {\"from\": 33250, \"to\": 36000}, \"text\": \" (music playing)\"},\n\
             {\"offsets\": {\"from\": 36000, \"to\": 37000}, \"text\": \" [Music] \u{266a}\"},\n\
             {\"offsets\": {\"from\": 37000, \"to\": 38000}, \"text\": \" *laughs*\"},\n\
             {\"offsets\": {\"from\": 38000, \"to\": 41500}, \"text\": \" Today:\tthe \\\"proof\\\".\"},\n\
             {\"offsets\": {\"from\": 41500, \"to\": 42000}, \"text\": \"  \"}\n\
             ]}",
        )
        .unwrap();
        assert_eq!(
            segments,
            vec![
                Segment {
                    start: 30.0,
                    end: 33.25,
                    text: "Good morning, everyone.".into()
                },
                Segment {
                    start: 38.0,
                    end: 41.5,
                    text: "Today: the \"proof\".".into()
                },
            ]
        );
        assert!(parse_reply(r#"{"transcription":[]}"#).unwrap().is_empty());
        assert!(matches!(
            parse_reply("whisper_init: loading"),
            Err(EngineError::Failed(_))
        ));
    }

    #[test]
    fn markers_are_dropped_but_speech_beside_them_is_kept() {
        assert!(non_speech("[BLANK_AUDIO]"));
        assert!(non_speech("[ Silence ]"));
        assert!(non_speech("(silence)"));
        assert!(non_speech("-"));
        assert!(!non_speech("(laughs) So the proof"));
        assert!(!non_speech("Thank you."));
        assert!(!non_speech("Ça va?"));
    }

    #[test]
    fn the_helpers_exit_and_output_decide_the_answer() {
        let ok = answer(
            Some(0),
            Some(r#"{"transcription":[{"offsets":{"from":1000,"to":2000},"text":" Hi."}]}"#),
            "",
        )
        .unwrap();
        assert_eq!(
            ok,
            vec![Segment {
                start: 1.0,
                end: 2.0,
                text: "Hi.".into()
            }]
        );
        // An unknown language exits 0, prints usage after the error, and writes nothing.
        assert!(matches!(
            answer(Some(0), None, "error: unknown language 'xx'\n\nusage: whisper-cli [options]\n  -h, --help\n"),
            Err(EngineError::Failed(m)) if m == "error: unknown language 'xx'"
        ));
        assert!(matches!(
            answer(Some(3), None, "error: failed to initialize whisper context\n"),
            Err(EngineError::Failed(m)) if m.contains("initialize whisper context")
        ));
        assert!(
            matches!(answer(Some(10), None, ""), Err(EngineError::Failed(m)) if m.contains("code 10"))
        );
        assert!(
            matches!(answer(None, None, ""), Err(EngineError::Failed(m)) if m.contains("killed"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_engine_runs_the_helper_on_a_wav_and_reads_its_json() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::test_support::Scratch::new("whisper-helper");
        // Stands in for whisper-cli: writes its arguments as the one segment's text.
        let script = dir.join("whisper-cli");
        std::fs::write(
            &script,
            "#!/bin/sh\nfor a; do case \"$prev\" in -of) out=\"$a\";; esac; prev=\"$a\"; done\n\
             printf '{\"transcription\":[{\"offsets\":{\"from\":0,\"to\":1500},\"text\":\" %s\"}]}' \"$*\" > \"$out.json\"\n",
        )
        .unwrap();
        // Stands in for ffmpeg: creates its last argument.
        let ffmpeg = dir.join("ffmpeg");
        std::fs::write(
            &ffmpeg,
            "#!/bin/sh\nfor a; do last=\"$a\"; done\n: > \"$last\"\n",
        )
        .unwrap();
        for path in [&script, &ffmpeg] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let audio = dir.join("audio.ogg");
        std::fs::write(&audio, b"x").unwrap();

        let whisper = Whisper {
            helper: script,
            ffmpeg,
            model: PathBuf::from("/models/ggml-tiny.bin"),
            vad: Some(PathBuf::from("/models/vad.bin")),
            language: "en".into(),
        };
        let found = whisper.transcribe(&audio).unwrap();
        let wav = dir.join("audio.wav");
        let text = &found[0].text;
        assert!(
            text.starts_with(&format!(
                "-m /models/ggml-tiny.bin -f {} -l en -t ",
                wav.display()
            )),
            "{text}"
        );
        assert!(
            text.ends_with(&format!(
                "-oj -np -sns -of {} --vad -vm /models/vad.bin",
                dir.join("audio.whisper").display()
            )),
            "{text}"
        );
        assert_eq!(found[0].end, 1.5);
        // The WAV and the JSON are cleaned up after each call.
        assert!(!wav.exists() && !dir.join("audio.whisper.json").exists());
        assert_eq!(whisper.max_upload_bytes(), None);
    }

    /// End to end on real files, outside the library and the database:
    /// `OCULUS_WHISPER_E2E=<video>` downloads `tiny` into a temp dir, extracts
    /// the video's audio and transcribes it with the dev whisper-cli.
    #[test]
    #[ignore]
    fn end_to_end_on_a_real_video() {
        let video =
            PathBuf::from(std::env::var("OCULUS_WHISPER_E2E").expect("OCULUS_WHISPER_E2E=<video>"));
        let models = crate::test_support::Scratch::new("whisper-e2e-models");
        let work = crate::test_support::Scratch::new("whisper-e2e-work");
        let tiny = whisper_models::find("tiny").unwrap();
        let started = std::time::Instant::now();
        let model = whisper_models::download(&models, tiny, |_, _| {}).unwrap();
        eprintln!("downloaded tiny + VAD in {:.1?}", started.elapsed());

        let ffmpeg = crate::sources::echo360::find_ffmpeg(None).expect("ffmpeg");
        let audio_path = work.join("audio.ogg");
        let extracted = audio::extract(&ffmpeg, &video, &audio_path).unwrap();
        let whisper = Whisper {
            helper: crate::runtime::bundled::find(HELPER, None).expect("whisper-cli"),
            ffmpeg,
            model,
            vad: Some(models.join(whisper_models::VAD_FILE)),
            language: "en".into(),
        };
        let started = std::time::Instant::now();
        let segments = whisper.transcribe(&audio_path).unwrap();
        eprintln!(
            "{:.0} s of audio in {:.1?}: {} segment(s)",
            extracted.duration.unwrap_or(0.0),
            started.elapsed(),
            segments.len()
        );
        for s in segments.iter().take(8) {
            eprintln!("{:8.2} → {:8.2}  {}", s.start, s.end, s.text);
        }
        assert!(!segments.is_empty());
    }
}

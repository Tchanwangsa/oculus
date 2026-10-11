//! `oculus transcribe`.

use crate::*;
use app_lib::transcribe::{self, Step};

impl Ctx {
    /// `transcribe::run`, the app's path too. Writes the `.vtt` and nothing else.
    pub(crate) fn transcribe(&self, args: &TranscribeArgs) -> Result<(), String> {
        // An agent's cwd is inside the library, so a path relative to it wins
        // when it exists; `transcribe::resolve` still refuses anything outside.
        let given = std::path::Path::new(&args.video);
        let video = match given
            .is_relative()
            .then(|| given.canonicalize().ok())
            .flatten()
        {
            Some(here) => here.to_string_lossy().into_owned(),
            None => args.video.clone(),
        };
        let resolved = transcribe::resolve(&self.data_dir, &video)?;
        let target = transcribe::vtt_path(&resolved);
        if target.exists() && !args.force {
            return Err(format!(
                "{} already exists — `--force` transcribes again and replaces it",
                target.display()
            ));
        }
        // No resource dir here: the dev copy in src-tauri/binaries, or the system's.
        let ffmpeg = app_lib::sources::echo360::find_ffmpeg(None)
            .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;

        let quiet = self.json;
        let only = args.engine.as_deref();
        let outcome = transcribe::run(&self.data_dir, &video, &ffmpeg, None, only, |step| {
            if quiet {
                return;
            }
            let line = match step {
                Step::Extracting => "extracting the audio".to_string(),
                Step::Transcribing {
                    engine, chunks: 1, ..
                } => {
                    format!("transcribing with {}", transcribe::engine_label(engine))
                }
                Step::Transcribing {
                    engine,
                    chunk,
                    chunks,
                } => format!(
                    "transcribing part {chunk} of {chunks} with {}",
                    transcribe::engine_label(engine)
                ),
            };
            println!("{}", paint(&line, DIM));
        })?;
        let path = outcome.path.to_string_lossy().into_owned();

        if self.json {
            #[derive(Serialize)]
            struct Out<'a> {
                video: &'a str,
                vtt: &'a str,
                engine: &'a str,
                chunks: usize,
                cues: usize,
            }
            return self.emit(&Out {
                video: &resolved.to_string_lossy(),
                vtt: &path,
                engine: outcome.engine,
                chunks: outcome.chunks,
                cues: outcome.cues,
            });
        }
        println!(
            "{}",
            paint(
                &format!(
                    "{} cue(s) written to {path}, by {}",
                    outcome.cues,
                    transcribe::engine_label(outcome.engine)
                ),
                DIM
            )
        );
        Ok(())
    }
}

//! `oculus studio narrate` and `oculus studio tts`: the TTS server, through
//! `app_lib::studio` (docs/studio.md, "Narration is made by `oculus studio
//! narrate`").

use crate::*;
use app_lib::studio::narrate::{self, ModelAligner, Outcome, Progress, SceneReport};
use app_lib::studio::tts;
use app_lib::studio::voice::{self, Voice, VoiceOverrides};
use studio_core::{Project, LOW_CONFIDENCE};

impl Ctx {
    pub(crate) fn studio_narrate(&self, args: &StudioNarrateArgs) -> Result<(), String> {
        let project = Project::load_unnarrated(&args.dir).map_err(|e| e.to_string())?;
        let video = project
            .video
            .voice
            .as_ref()
            .map(VoiceOverrides::from)
            .unwrap_or_default();
        let voice = voice::resolve(&flags(&args.voice), &video, &voice::settings())?;
        let options = narrate::Options {
            scene: args.scene.clone(),
            force: args.force,
        };
        let quiet = self.json;
        if !quiet {
            println!(
                "{}",
                paint(
                    &format!(
                        "voice {} · {} · speed {} · {}",
                        voice.voice, voice.model, voice.speed, voice.base_url
                    ),
                    DIM
                )
            );
        }
        let started = std::time::Instant::now();
        let report = narrate::narrate(&args.dir, &voice, &ModelAligner, &options, &mut |p| {
            if !quiet {
                print_progress(&p);
            }
        })
        .map_err(|e| e.to_string())?;
        let ms = started.elapsed().as_millis();

        if self.json {
            return self.emit(&serde_json::json!({
                "dir": args.dir.to_string_lossy(),
                "voice": voice_json(&voice),
                "scenes": report.scenes,
                "dropped": report.dropped,
                "deleted": report.deleted,
                "low_confidence_below": LOW_CONFIDENCE,
                "ms": ms,
            }));
        }
        for id in &report.dropped {
            println!(
                "{}",
                paint(&format!("  dropped `{id}`: no longer narrated"), DIM)
            );
        }
        if !report.deleted.is_empty() {
            println!(
                "{}",
                paint(
                    &format!(
                        "  deleted {} unused clip{}",
                        report.deleted.len(),
                        if report.deleted.len() == 1 { "" } else { "s" }
                    ),
                    DIM
                )
            );
        }
        let narrated = report
            .scenes
            .iter()
            .filter(|s| s.outcome == Outcome::Narrated)
            .count();
        println!(
            "{} {} scene{} ({narrated} narrated, {} cached) in {:.1} s",
            paint("done", GREEN),
            report.scenes.len(),
            if report.scenes.len() == 1 { "" } else { "s" },
            report.scenes.len() - narrated,
            ms as f64 / 1000.0
        );
        Ok(())
    }

    pub(crate) fn studio_tts(&self, args: &StudioTtsArgs) -> Result<(), String> {
        let voice = voice::resolve(
            &flags(&args.voice),
            &VoiceOverrides::default(),
            &voice::settings(),
        )?;
        let started = std::time::Instant::now();
        let speech = tts::synthesise(&voice, &args.text)?;
        let ms = started.elapsed().as_millis();
        std::fs::write(&args.out, &speech.wav)
            .map_err(|e| format!("cannot write {}: {e}", args.out.display()))?;
        let d = &speech.decoded;
        if self.json {
            return self.emit(&serde_json::json!({
                "path": args.out.to_string_lossy(),
                "duration": d.duration,
                "sample_rate": d.sample_rate,
                "voice": voice_json(&voice),
                "ms": ms,
            }));
        }
        println!("{} {}", paint("wrote", GREEN), args.out.display());
        println!(
            "{}",
            paint(
                &format!(
                    "  {:.2} s at {} Hz · {} · {} · speed {} · {ms} ms",
                    d.duration, d.sample_rate, voice.voice, voice.model, voice.speed
                ),
                DIM
            )
        );
        if d.duration > studio_core::MAX_NARRATION_SECS {
            println!(
                "{} over {} s: `narrate` refuses a clip this long; split the text",
                paint("warning:", YELLOW),
                studio_core::MAX_NARRATION_SECS
            );
        }
        Ok(())
    }
}

fn flags(args: &StudioVoiceArgs) -> VoiceOverrides {
    VoiceOverrides {
        base_url: args.server.clone(),
        model: args.model.clone(),
        voice: args.voice.clone(),
        speed: args.speed,
    }
}

fn voice_json(voice: &Voice) -> serde_json::Value {
    serde_json::json!({
        "server": voice.base_url,
        "model": voice.model,
        "voice": voice.voice,
        "speed": voice.speed,
    })
}

fn print_progress(p: &Progress) {
    match p {
        Progress::Reset(why) => println!(
            "{} narration/manifest.json is unreadable ({why}); rebuilding it",
            paint("warning:", YELLOW)
        ),
        Progress::Start { .. } => {}
        Progress::Done(scene) => print_scene(scene),
    }
}

/// `id  narrated  3.42 s  11 words  (synth 0.4 s, align 1.2 s)`, then a
/// warning line per unsure word and per gap.
fn print_scene(s: &SceneReport) {
    let (outcome, colour) = match s.outcome {
        Outcome::Narrated => ("narrated", GREEN),
        Outcome::Cached => ("cached", DIM),
    };
    let timings = s
        .timings
        .map(|t| {
            format!(
                "  (synth {:.1} s, align {:.1} s)",
                t.synth_ms as f64 / 1000.0,
                t.align_ms as f64 / 1000.0
            )
        })
        .unwrap_or_default();
    println!(
        "{} {} {:>6.2} s  {:>3} words{}",
        paint(&format!("{:<16}", s.id), BOLD),
        paint(&format!("{outcome:<8}"), colour),
        s.duration,
        s.words,
        paint(&timings, DIM)
    );
    for w in &s.low_confidence {
        println!(
            "  {} the aligner is unsure of `{}` at {:.2} s into the clip (confidence {:.2}, below {LOW_CONFIDENCE}); listen to it, and reword or re-narrate",
            paint("warning:", YELLOW),
            w.text,
            w.start,
            w.confidence
        );
    }
    for g in &s.gaps {
        let place = match &g.after_word {
            Some(w) => format!("after `{w}`"),
            None => "before the first word".into(),
        };
        println!(
            "  {} unscripted speech {place}, {:.2} to {:.2} s into the clip (heard \"{}\")",
            paint("warning:", YELLOW),
            g.start,
            g.end,
            g.heard
        );
    }
}

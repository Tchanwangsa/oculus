//! `oculus studio narrate` and `oculus studio tts`: the TTS server or Kokoro,
//! through `app_lib::studio` (docs/studio.md, "Narration is made by `oculus
//! studio narrate`").

use crate::*;
use app_lib::studio::libraries;
use app_lib::studio::narrate::{
    self, ModelAligner, Outcome, Progress, SceneReport, ServerSpeaker, Speaker,
};
use app_lib::studio::tts::kokoro::{KokoroSpeaker, LexiconFiles, Packs};
use app_lib::studio::voice::lint::{notes, Note};
use app_lib::studio::voice::review::Store;
use app_lib::studio::voice::{self, Engine, Voice, VoiceOverrides};
use studio_core::{Project, LOW_CONFIDENCE};

impl Ctx {
    pub(crate) fn studio_narrate(&self, args: &StudioNarrateArgs) -> Result<(), String> {
        let library = libraries::libraries(false).map_err(|e| e.to_string())?;
        let project =
            Project::load_unnarrated_with(&args.dir, &library).map_err(|e| e.to_string())?;
        let video = project
            .video
            .voice
            .as_ref()
            .map(VoiceOverrides::from)
            .unwrap_or_default();
        let voice = voice::resolve(&flags(&args.voice), &video, &voice::settings())?;
        let speaker = speaker(&voice, project.video.subject.as_deref());
        let options = narrate::Options {
            scene: args.scene.clone(),
            force: args.force,
        };
        let quiet = self.json;
        if !quiet {
            println!("{}", paint(&describe(&voice), DIM));
        }
        let started = std::time::Instant::now();
        let report = narrate::narrate(
            &args.dir,
            &library,
            speaker.speaker(),
            &ModelAligner,
            &options,
            &mut |p| {
                if !quiet {
                    print_progress(&p);
                }
            },
        )
        .map_err(|e| e.to_string())?;
        let ms = started.elapsed().as_millis();
        let mut notes = speaker.notes(&project);
        Store::app().annotate(project.video.subject.as_deref(), &mut notes);

        if self.json {
            return self.emit(&serde_json::json!({
                "dir": args.dir.to_string_lossy(),
                "voice": voice_json(&voice),
                "scenes": report.scenes,
                "notes": notes,
                "dropped": report.dropped,
                "deleted": report.deleted,
                "low_confidence_below": LOW_CONFIDENCE,
                "ms": ms,
            }));
        }
        for note in &notes {
            print_note(note);
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
        let speaker = speaker(&voice, None);
        speaker.speaker().ready()?;
        let started = std::time::Instant::now();
        let speech = speaker.speaker().speak(&args.text)?;
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
                    "  {:.2} s at {} Hz · {} · {ms} ms",
                    d.duration,
                    d.sample_rate,
                    describe(&voice)
                ),
                DIM
            )
        );
        Ok(())
    }
}

/// The speaker `voice`'s engine makes clips with; Kokoro reads the
/// lexicon of `subject` over the library's.
fn speaker<'a>(voice: &'a Voice, subject: Option<&str>) -> Speakers<'a> {
    match voice {
        Voice::Server(v) => Speakers::Server(ServerSpeaker(v)),
        Voice::Kokoro(v) => Speakers::Kokoro(Box::new(KokoroSpeaker::new(
            v.clone(),
            Packs::app(),
            LexiconFiles::app(subject),
        ))),
    }
}

enum Speakers<'a> {
    Server(ServerSpeaker<'a>),
    Kokoro(Box<KokoroSpeaker>),
}

impl Speakers<'_> {
    fn speaker(&self) -> &dyn Speaker {
        match self {
            Speakers::Server(s) => s,
            Speakers::Kokoro(s) => &**s,
        }
    }

    /// A note per word Kokoro guessed in the scenes it read.
    fn notes(&self, project: &Project) -> Vec<Note> {
        match self {
            Speakers::Server(_) => Vec::new(),
            Speakers::Kokoro(k) => notes(project, |text| k.phonemized(text)),
        }
    }
}

/// `note: scene 2 `momentum` script.md:9: check pronunciation: ...`
pub(super) fn print_note(note: &Note) {
    println!(
        "{} {} {}",
        paint("note:", BOLD),
        paint(
            &format!(
                "scene {} `{}` script.md:{}:",
                note.scene, note.id, note.line
            ),
            BOLD
        ),
        note.message()
    );
}

/// `voice af_heart · kokoro · speed 1`, or the server's model and URL.
fn describe(voice: &Voice) -> String {
    match voice {
        Voice::Server(v) => format!(
            "voice {} · {} · speed {} · {}",
            v.voice, v.model, v.speed, v.base_url
        ),
        Voice::Kokoro(v) => format!("voice {} · kokoro · speed {}", v.voice, v.speed),
    }
}

fn flags(args: &StudioVoiceArgs) -> VoiceOverrides {
    VoiceOverrides {
        engine: args.engine.map(|e| match e {
            StudioEngineArg::Kokoro => Engine::Kokoro,
            StudioEngineArg::Server => Engine::Server,
        }),
        base_url: args.server.clone(),
        model: args.model.clone(),
        voice: args.voice.clone(),
        speed: args.speed,
    }
}

fn voice_json(voice: &Voice) -> serde_json::Value {
    match voice {
        Voice::Server(v) => serde_json::json!({
            "engine": "server",
            "server": v.base_url,
            "model": v.model,
            "voice": v.voice,
            "speed": v.speed,
        }),
        Voice::Kokoro(v) => serde_json::json!({
            "engine": "kokoro",
            "voice": v.voice,
            "speed": v.speed,
        }),
    }
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

/// `id  narrated  3.42 s  11 words  2 pauses  (synth 0.4 s, align 1.2 s)`,
/// then a warning line per unsure word and per gap.
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
    let pauses = match s.pauses {
        0 => String::new(),
        1 => "  1 pause".to_owned(),
        n => format!("  {n} pauses"),
    };
    println!(
        "{} {} {:>6.2} s  {:>3} words{pauses}{}",
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

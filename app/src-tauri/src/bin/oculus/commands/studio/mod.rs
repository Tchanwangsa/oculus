//! `oculus studio`: make, render and check a Studio video folder with
//! `studio_core` (docs/studio.md). A DIR argument that is not a folder but
//! a bare slug names a video under `<app data>/studio/videos/`. `frame`,
//! `strip` and `lint` read only the folder and write only the PNG (`-o`
//! makes its folder); the other commands are in their own files.

mod agents;
mod aligner;
mod brief;
mod components;
mod g2p;
mod gallery;
mod narrate;
mod render;
mod themes;
mod videos;
mod voices;

use crate::*;
use app_lib::studio::tts::kokoro::{self, LexiconFiles};
use app_lib::studio::voice::lint::{Note, VoiceLint};
use app_lib::studio::voice::{self, VoiceOverrides};
use app_lib::studio::{libraries, videos as video_dirs};
use std::path::Path;
use std::time::Instant;
use studio_core::{
    LintIssue, MotionBlur, RenderOptions, Renderer, StripOptions, StudioError, Time,
};

impl Ctx {
    /// Resolves the command's DIR (a folder, or a video's slug), then runs it.
    pub(crate) fn studio(&self, mut action: StudioAction) -> Result<(), String> {
        if let Some(dir) = action.dir_mut() {
            *dir = video_dirs::resolve(dir)?;
        }
        match action {
            StudioAction::New(a) => self.studio_new(&a),
            StudioAction::List(a) => self.studio_list(&a),
            StudioAction::Frame(a) => self.studio_frame(&a),
            StudioAction::Strip(a) => self.studio_strip(&a),
            StudioAction::Lint(a) => self.studio_lint(&a),
            StudioAction::Brief(a) => self.studio_brief(&a),
            StudioAction::Gallery(a) => self.studio_gallery(&a),
            StudioAction::Components { action } => self.studio_components(&action),
            StudioAction::Themes { action } => self.studio_themes(&action),
            StudioAction::Render(a) => self.studio_render(&a),
            StudioAction::Narrate(a) => self.studio_narrate(&a),
            StudioAction::Tts(a) => self.studio_tts(&a),
            StudioAction::G2p(a) => self.studio_g2p(&a),
            StudioAction::Voices { action } => self.studio_voices(&action),
            StudioAction::Agents(a) => self.studio_agents(&a),
            StudioAction::Aligner { action } => self.studio_aligner(&action),
        }
    }

    pub(crate) fn studio_frame(&self, args: &StudioFrameArgs) -> Result<(), String> {
        let project = libraries::open(&args.dir, None, None).map_err(studio_error)?;
        let at = match args.scene {
            Some(index) => Time::Scene {
                index,
                t: args.at.unwrap_or(0.0),
            },
            None => Time::Global(args.t.unwrap_or(0.0)),
        };
        let timeline = project.timeline().map_err(studio_error)?;
        let (scene, t) = timeline.locate(at).map_err(studio_error)?;
        let global = timeline.start(scene) + t;
        let component = project.video.scenes[scene].component.clone();

        let out = png_out(args.out.as_ref(), &args.dir, || {
            format!("frame-{}.png", seconds(global))
        })?;

        let options = RenderOptions {
            scale: args.scale,
            motion_blur: args.motion_blur.then(|| MotionBlur::at(project.fps())),
        };
        let mut renderer = Renderer::new(&project, options).map_err(studio_error)?;
        let started = Instant::now();
        let frame = renderer.frame(at).map_err(studio_error)?;
        let ms = started.elapsed().as_millis();
        frame.write_png(&out).map_err(studio_error)?;

        if self.json {
            return self.emit(&serde_json::json!({
                "path": out.to_string_lossy(),
                "scene": scene,
                "component": component,
                "t": t,
                "width": frame.width,
                "height": frame.height,
                "ms": ms,
                "samples": frame.samples,
                "logs": frame.logs,
            }));
        }
        println!("{} {}", paint("wrote", GREEN), out.display());
        let samples = match frame.samples {
            1 => String::new(),
            n => format!(" · {n} sub-frames"),
        };
        println!(
            "{}",
            paint(
                &format!(
                    "  scene {scene} ({component}), {t:.3} s in · {}×{} · {ms} ms{samples}",
                    frame.width, frame.height
                ),
                DIM
            )
        );
        for line in &frame.logs {
            println!("{}", paint(&format!("  log: {line}"), DIM));
        }
        Ok(())
    }

    pub(crate) fn studio_strip(&self, args: &StudioStripArgs) -> Result<(), String> {
        let project = libraries::open(&args.dir, None, None).map_err(studio_error)?;
        let (from, to) = match args.scene {
            Some(index) => project
                .timeline()
                .and_then(|tl| tl.window(index))
                .map_err(studio_error)?,
            None => (args.from.unwrap_or(0.0), args.to.unwrap_or(f64::INFINITY)),
        };
        let options = StripOptions {
            from,
            to,
            n: args.n,
            cols: args.cols,
            scale: args.scale,
            threads: args.threads,
            motion_blur: args.motion_blur.then(|| MotionBlur::at(project.fps())),
        };
        let started = Instant::now();
        let strip = studio_core::strip(&project, options).map_err(studio_error)?;
        let ms = started.elapsed().as_millis();

        let out = png_out(args.out.as_ref(), &args.dir, || {
            let last = strip.times.last().copied().unwrap_or(from);
            format!("strip-{}-{}.png", seconds(from), seconds(last))
        })?;
        strip.write_png(&out).map_err(studio_error)?;

        if self.json {
            return self.emit(&serde_json::json!({
                "path": out.to_string_lossy(),
                "times": strip.times,
                "width": strip.width,
                "height": strip.height,
                "tile": { "width": strip.tile.0, "height": strip.tile.1 },
                "cols": strip.cols,
                "ms": ms,
                "threads": strip.threads,
            }));
        }
        println!("{} {}", paint("wrote", GREEN), out.display());
        println!(
            "{}",
            paint(
                &format!(
                    "  {} frames, {} to {} s · {}×{} ({}×{} tiles) · {ms} ms on {} thread{}",
                    strip.times.len(),
                    seconds(strip.times.first().copied().unwrap_or(from)),
                    seconds(strip.times.last().copied().unwrap_or(from)),
                    strip.width,
                    strip.height,
                    strip.tile.0,
                    strip.tile.1,
                    strip.threads,
                    if strip.threads == 1 { "" } else { "s" },
                ),
                DIM
            )
        );
        Ok(())
    }

    pub(crate) fn studio_lint(&self, args: &StudioLintArgs) -> Result<(), String> {
        let project = libraries::open(&args.dir, None, None).map_err(studio_error)?;
        let mut report = project.lint().map_err(studio_error)?;
        let spoken = voice_lint(&project);
        report.issues.extend(spoken.issues);
        let count = report.issues.len();

        if self.json {
            let entries: Vec<serde_json::Value> = report
                .issues
                .iter()
                .map(issue_json)
                .chain(spoken.notes.iter().map(note_json))
                .collect();
            self.emit(&entries)?;
        } else if report.is_clean() {
            println!(
                "{} {} ({} scenes, {:.3} s)",
                paint("no problems", GREEN),
                args.dir.display(),
                project.video.scenes.len(),
                project.length().map_err(studio_error)?
            );
        } else {
            for issue in &report.issues {
                print_issue(issue);
            }
        }
        if !self.json {
            for note in &spoken.notes {
                narrate::print_note(note);
            }
        }
        if count == 0 {
            return Ok(());
        }
        Err(format!(
            "{count} problem{} in {}",
            if count == 1 { "" } else { "s" },
            args.dir.display()
        ))
    }
}

/// The checks that need the narration engine (`voice::lint`), with the
/// voice `narrate` would resolve without flags. A voice that does not
/// resolve is `narrate`'s to report.
fn voice_lint(project: &studio_core::Project) -> VoiceLint {
    let block = project
        .video
        .voice
        .as_ref()
        .map(VoiceOverrides::from)
        .unwrap_or_default();
    match voice::resolve(&VoiceOverrides::default(), &block, &voice::settings()) {
        Ok(v) => voice::lint::lint(
            project,
            &v,
            &kokoro::dir(),
            &LexiconFiles::app(project.video.subject.as_deref()),
        ),
        Err(_) => VoiceLint::default(),
    }
}

fn note_json(note: &Note) -> serde_json::Value {
    serde_json::json!({
        "level": "note",
        "scene": note.scene,
        "id": note.id,
        "line": note.line,
        "kind": "guessed",
        "message": note.message(),
        "word": note.word,
        "phonemes": note.phonemes,
        "guessed": note.guessed,
    })
}

/// One issue: where it is, then what is wrong. A script issue with no
/// scene names its own place (`script.md:7: ...`).
fn print_issue(issue: &LintIssue) {
    let id = issue
        .id
        .as_ref()
        .map(|id| format!(" `{id}`"))
        .unwrap_or_default();
    let component = issue.component.as_deref();
    let place = match (issue.scene, &issue.example, component) {
        (Some(i), _, Some(c)) => format!("scene {i}{id} ({c})"),
        (Some(i), _, None) => format!("scene {i}{id}"),
        (None, Some(e), Some(c)) => format!("{c} example `{e}`"),
        (None, _, Some(c)) => c.to_owned(),
        (None, _, None) => {
            println!("{}", issue.error);
            return;
        }
    };
    println!("{} {}", paint(&place, BOLD), issue.error);
}

fn issue_json(issue: &LintIssue) -> serde_json::Value {
    let e = &issue.error;
    serde_json::json!({
        "level": "issue",
        "scene": issue.scene,
        "id": issue.id,
        "component": issue.component,
        "example": issue.example,
        "kind": e.kind(),
        "message": e.to_string(),
        "loc": e.loc().map(|l| serde_json::json!({
            "line": l.line,
            "col": l.col,
            "snippet": l.snippet.trim(),
        })),
    })
}

/// An error's own message: studio-core writes them for agents to act on.
fn studio_error(e: StudioError) -> String {
    e.to_string()
}

/// `-o`'s path, else `<DIR>/out/<name>`, with its folder made.
fn png_out(
    out: Option<&PathBuf>,
    dir: &Path,
    name: impl FnOnce() -> String,
) -> Result<PathBuf, String> {
    let path = out.cloned().unwrap_or_else(|| dir.join("out").join(name()));
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    Ok(path)
}

/// Seconds for a filename: up to three decimals, trailing zeros dropped.
fn seconds(t: f64) -> String {
    let s = format!("{t:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

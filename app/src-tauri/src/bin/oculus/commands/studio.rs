//! `oculus studio`: render and check a Studio video folder with
//! `studio_core` (docs/studio.md). Reads only the folder; writes only the PNG.

use crate::*;
use std::time::Instant;
use studio_core::{
    LintIssue, MotionBlur, Project, RenderOptions, Renderer, StripOptions, StudioError, Time,
};

impl Ctx {
    pub(crate) fn studio_frame(&self, args: &StudioFrameArgs) -> Result<(), String> {
        let project = Project::load(&args.dir).map_err(studio_error)?;
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

        let out = match &args.out {
            Some(path) => path.clone(),
            None => {
                let dir = args.dir.join("out");
                std::fs::create_dir_all(&dir)
                    .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
                dir.join(format!("frame-{}.png", seconds(global)))
            }
        };

        let options = RenderOptions {
            scale: args.scale,
            motion_blur: args.motion_blur.then(MotionBlur::default),
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
        let project = Project::load(&args.dir).map_err(studio_error)?;
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
            motion_blur: args.motion_blur.then(MotionBlur::default),
        };
        let started = Instant::now();
        let strip = studio_core::strip(&project, options).map_err(studio_error)?;
        let ms = started.elapsed().as_millis();

        let out = match &args.out {
            Some(path) => path.clone(),
            None => {
                let dir = args.dir.join("out");
                std::fs::create_dir_all(&dir)
                    .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
                let last = strip.times.last().copied().unwrap_or(from);
                dir.join(format!("strip-{}-{}.png", seconds(from), seconds(last)))
            }
        };
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
        let project = Project::load(&args.dir).map_err(studio_error)?;
        let report = project.lint().map_err(studio_error)?;
        let count = report.issues.len();

        if self.json {
            let issues: Vec<serde_json::Value> = report.issues.iter().map(issue_json).collect();
            self.emit(&issues)?;
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

/// Seconds for a filename: up to three decimals, trailing zeros dropped.
fn seconds(t: f64) -> String {
    let s = format!("{t:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

//! `oculus studio`: render and check a Studio video folder with
//! `studio_core` (docs/studio.md). Reads only the folder; writes only the PNG.

use crate::*;
use std::time::Instant;
use studio_core::{LintIssue, Project, RenderOptions, Renderer, StudioError, Time};

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
        let timeline = project.timeline();
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

        let mut renderer =
            Renderer::new(&project, RenderOptions { scale: args.scale }).map_err(studio_error)?;
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
                "logs": frame.logs,
            }));
        }
        println!("{} {}", paint("wrote", GREEN), out.display());
        println!(
            "{}",
            paint(
                &format!(
                    "  scene {scene} ({component}), {t:.3} s in · {}×{} · {ms} ms",
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
                project.length()
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

/// One issue: where it is, then what is wrong.
fn print_issue(issue: &LintIssue) {
    let place = match (issue.scene, &issue.example) {
        (Some(i), _) => format!("scene {i} ({})", issue.component),
        (None, Some(e)) => format!("{} example `{e}`", issue.component),
        (None, None) => issue.component.clone(),
    };
    println!("{} {}", paint(&place, BOLD), issue.error);
}

fn issue_json(issue: &LintIssue) -> serde_json::Value {
    let e = &issue.error;
    serde_json::json!({
        "scene": issue.scene,
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

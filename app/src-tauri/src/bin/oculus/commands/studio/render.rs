//! `oculus studio render`: a video folder to an mp4 through
//! `app_lib::studio::render` (docs/studio.md, "Rendering a video").

use crate::*;
use app_lib::studio::render::{self, Finished, Outcome, Progress, Quality, SegmentReport};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

/// How often a segment's progress line is printed.
const EVERY: Duration = Duration::from_secs(2);

impl Ctx {
    pub(crate) fn studio_render(&self, args: &StudioRenderArgs) -> Result<(), String> {
        let quality = if args.final_ {
            Quality::Final
        } else {
            Quality::Draft
        };
        let ffmpeg = app_lib::sources::echo360::find_ffmpeg(None)
            .ok_or("ffmpeg not found; run `bun run ffmpeg` in app/, or install it")?;
        let options = render::Options {
            quality,
            threads: args.threads,
            out: args.out.clone(),
            ffmpeg,
        };
        let quiet = self.json;
        let mut last = Instant::now();
        let finished = render::render(&args.dir, &options, &AtomicBool::new(false), &mut |p| {
            if !quiet {
                print_progress(&p, &mut last);
            }
        })
        .map_err(|e| e.to_string())?;
        let report = match finished {
            Finished::Complete(report) => report,
            Finished::Cancelled => return Err("cancelled".into()),
        };

        if self.json {
            return self.emit(&serde_json::json!({
                "dir": args.dir.to_string_lossy(),
                "report": report,
            }));
        }
        let rendered: Vec<&SegmentReport> = report
            .segments
            .iter()
            .filter(|s| s.outcome == Outcome::Rendered)
            .collect();
        let frames: u64 = rendered.iter().map(|s| s.frames).sum();
        let ms: u64 = rendered.iter().map(|s| s.ms).sum();
        let rate = if ms > 0 {
            format!(", {:.1} frames/s", frames as f64 * 1000.0 / ms as f64)
        } else {
            String::new()
        };
        println!("{} {}", paint("wrote", GREEN), report.path.display());
        println!(
            "{}",
            paint(
                &format!(
                    "  {} · {}×{} at {} fps · {} frames, {:.2} s{} · {} rendered{rate}, {} cached · {} thread{} · {:.1} s",
                    report.quality.name(),
                    report.width,
                    report.height,
                    report.fps,
                    report.frames,
                    report.duration,
                    if report.audio { " with narration" } else { "" },
                    rendered.len(),
                    report.segments.len() - rendered.len(),
                    report.threads,
                    if report.threads == 1 { "" } else { "s" },
                    report.ms as f64 / 1000.0
                ),
                DIM
            )
        );
        Ok(())
    }
}

fn print_progress(p: &Progress, last: &mut Instant) {
    match p {
        Progress::Planned {
            segments,
            cached,
            frames,
            threads,
        } => println!(
            "{}",
            paint(
                &format!(
                    "{segments} scene{}: {cached} cached, {frames} frames to render on {threads} thread{}",
                    if *segments == 1 { "" } else { "s" },
                    if *threads == 1 { "" } else { "s" },
                ),
                DIM
            )
        ),
        Progress::Segment { .. } => *last = Instant::now(),
        Progress::Frame {
            n,
            frame,
            frames,
            fps,
        } => {
            if last.elapsed() >= EVERY && frame < frames {
                *last = Instant::now();
                println!(
                    "{}",
                    paint(
                        &format!("  scene {}: frame {frame} of {frames}, {fps:.1} frames/s", n - 1),
                        DIM
                    )
                );
            }
        }
        Progress::Done(s) => print_segment(s),
        Progress::Muxing => println!("{}", paint("joining the segments", DIM)),
    }
}

/// `scene 2 `steps`  rendered  116 frames  3.9 s  29.7 frames/s`.
fn print_segment(s: &SegmentReport) {
    let id =
        s.id.as_ref()
            .map(|id| format!(" `{id}`"))
            .unwrap_or_default();
    let (outcome, colour) = match s.outcome {
        Outcome::Rendered => ("rendered", GREEN),
        Outcome::Cached => ("cached", DIM),
    };
    let timing = match s.fps {
        Some(fps) => format!("  {:.1} s  {fps:.1} frames/s", s.ms as f64 / 1000.0),
        None => String::new(),
    };
    println!(
        "{} {} {:>5} frames{}",
        paint(&format!("{:<20}", format!("scene {}{id}", s.scene)), BOLD),
        paint(&format!("{outcome:<8}"), colour),
        s.frames,
        paint(&timing, DIM)
    );
}

//! The writer side of a render: frames arrive in order from the workers,
//! each segment not cached gets its own ffmpeg, and every segment, cached or
//! not, is reported in video order.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use studio_core::{Frame, Project, RenderOptions};

use super::encoder::Encoder;
use super::error::Stop;
use super::plan::Segment;
use super::progress::{Outcome, Progress, SegmentReport};
use super::quality::Settings;
use super::workers::{self, Job};
use super::Options;

/// The segment being encoded.
struct Current {
    encoder: Encoder,
    index: usize,
    written: u64,
    /// When the segment before it finished (or the render began): workers
    /// start on a segment's frames before its first one is written.
    started: Instant,
}

/// Render and encode every segment not cached, reporting all of them in
/// video order.
#[allow(clippy::too_many_arguments)]
pub(super) fn encode(
    project: &Project,
    render_options: RenderOptions,
    segments: &[Segment],
    jobs: &[Job],
    threads: usize,
    settings: &Settings,
    options: &Options,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Vec<SegmentReport>, Stop> {
    let mut reports = Vec::with_capacity(segments.len());
    let mut cursor = 0;
    let of = segments.len();
    let skip_cached = |cursor: &mut usize,
                       reports: &mut Vec<SegmentReport>,
                       progress: &mut dyn FnMut(Progress)| {
        while let Some(s) = segments.get(*cursor).filter(|s| s.cached) {
            let report = SegmentReport {
                scene: s.scene,
                id: s.id.clone(),
                outcome: Outcome::Cached,
                frames: s.len(),
                ms: 0,
                fps: None,
            };
            progress(Progress::Done(&report));
            reports.push(report);
            *cursor += 1;
        }
    };

    if !jobs.is_empty() {
        let mut current: Option<Current> = None;
        let mut mark = Instant::now();
        workers::render(
            project,
            render_options,
            jobs,
            threads,
            cancel,
            |_, frame: Frame| {
                if current.is_none() {
                    skip_cached(&mut cursor, &mut reports, progress);
                    let s = &segments[cursor];
                    progress(Progress::Segment {
                        n: cursor + 1,
                        of,
                        scene: s.scene,
                    });
                    current = Some(Current {
                        encoder: Encoder::spawn(&options.ffmpeg, settings, &s.path)?,
                        index: cursor,
                        written: 0,
                        started: mark,
                    });
                }
                let c = current.as_mut().expect("a segment is open");
                c.encoder.write(&frame.rgba)?;
                c.written += 1;
                let s = &segments[c.index];
                let secs = c.started.elapsed().as_secs_f64();
                progress(Progress::Frame {
                    n: c.index + 1,
                    frame: c.written,
                    frames: s.len(),
                    fps: if secs > 0.0 {
                        c.written as f64 / secs
                    } else {
                        0.0
                    },
                });
                if c.written == s.len() {
                    let c = current.take().expect("a segment is open");
                    c.encoder.finish()?;
                    mark = Instant::now();
                    let secs = (mark - c.started).as_secs_f64();
                    let report = SegmentReport {
                        scene: s.scene,
                        id: s.id.clone(),
                        outcome: Outcome::Rendered,
                        frames: s.len(),
                        ms: (secs * 1000.0) as u64,
                        fps: Some(s.len() as f64 / secs),
                    };
                    progress(Progress::Done(&report));
                    reports.push(report);
                    cursor += 1;
                }
                Ok(())
            },
        )?;
    }
    skip_cached(&mut cursor, &mut reports, progress);
    Ok(reports)
}

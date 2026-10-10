//! Running the whole job.
//!
//! One implementation for the CLI and the app; they differ only in where the
//! agent selection comes from and how progress is shown.

use super::agent::{outline, parse_chapters, prompt, validate, Chapter, Job};
use super::frames::extract_frames;
use super::stream::detect;
use super::transcript::{cue_gaps, parse_transcript};
use super::Run;

/// The pipeline's own phases, for a caller that draws progress; what happens
/// inside the agent turn arrives on `on_event` instead.
pub enum Step<'a> {
    /// Fires often; [`run`] throttles it.
    Decoding {
        second: u32,
        duration: u32,
    },
    Detected {
        title: &'a str,
        duration: u32,
        candidates: usize,
    },
    Grabbing {
        done: usize,
        total: usize,
    },
    Asking,
    Writing,
}

pub struct Outcome {
    pub title: String,
    pub duration_seconds: u32,
    /// Second 0 not counted.
    pub candidates: usize,
    /// A 2 here is the only visible sign that source 1 was dead.
    pub source: crate::sources::echo360::SourceNum,
    pub chapters: Vec<Chapter>,
}

/// Detect, grab, ask, validate, write. Blocking and minutes long, so callers
/// run it off any thread that must stay responsive.
///
/// `chapter_status` is `running` from the start of the work, then `error` with
/// the message or `ready` (stamped by `store::save_chapters`). The guards
/// before it fail without claiming, so a refusal leaves no status behind.
pub fn run(
    rt: &tokio::runtime::Handle,
    pool: &sqlx::SqlitePool,
    job: &Run,
    on_step: impl Fn(Step),
    on_event: impl Fn(&crate::harness::HarnessEvent) + Send + Sync + 'static,
) -> Result<Outcome, String> {
    let id = job.lecture_id;
    let source = rt.block_on(crate::lectures::lecture_jobs::source(pool, id))?;
    let title = &source.title;
    let duration = source.duration;
    let transcript = &source.transcript;
    let code = &source.code;

    let existing = rt.block_on(crate::db::store::chapters(pool, id))?;
    if !existing.is_empty() && !job.force {
        return Err(format!(
            "{title} already has {} chapter(s) — re-running replaces them",
            existing.len()
        ));
    }
    let video = source.video()?;
    let ffmpeg = crate::sources::echo360::find_ffmpeg(None)
        .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;

    // Claimed before the decode, so the UI shows the run immediately.
    rt.block_on(crate::db::store::set_chapter_status(
        pool,
        id,
        Some("running"),
        None,
    ))?;

    let outcome = (|| -> Result<Outcome, String> {
        // A missing transcript is not fatal: chaptering works off the picture.
        let vtt = transcript
            .as_deref()
            .and_then(|p| std::fs::read_to_string(p).ok());
        let gaps = vtt.as_deref().map(cue_gaps).unwrap_or_default();
        let cues = vtt.as_deref().map(parse_transcript).unwrap_or_default();

        let dir = crate::sources::echo360::lecture_dir(job.data_dir, id);
        let mut last = std::time::Instant::now();
        let detected = detect(
            &ffmpeg,
            &dir,
            &video,
            &gaps,
            duration,
            job.source,
            |second| {
                if last.elapsed() >= std::time::Duration::from_millis(250) {
                    last = std::time::Instant::now();
                    on_step(Step::Decoding { second, duration });
                }
            },
        )?;
        let found = detected.candidates;
        if found.is_empty() {
            return Err(format!(
                "no boundary candidates in {title} — nothing to chapter"
            ));
        }
        on_step(Step::Detected {
            title: &title,
            duration,
            candidates: found.len(),
        });

        // Second 0 is never detected but is always a boundary.
        let mut frames_at: Vec<u32> = vec![0];
        frames_at.extend(found.iter().map(|c| c.seconds));

        // Frames only at slide changes, but any cue start is a boundary too.
        let mut boundaries = frames_at.clone();
        boundaries.extend(cues.iter().map(|c| c.start.max(0.0) as u32));
        boundaries.sort_unstable();
        boundaries.dedup();

        std::fs::write(dir.join("outline.md"), outline(&title, &cues, &found))
            .map_err(|e| format!("{}: {e}", dir.join("outline.md").display()))?;

        let total = frames_at.len();
        extract_frames(
            &ffmpeg,
            &detected.video,
            &frames_at,
            &dir.join("frames"),
            |done| on_step(Step::Grabbing { done, total }),
        )?;

        let course_dir = code
            .as_deref()
            .map(|c| format!("../courses/{}", crate::library::paths::safe_dir(c)));

        let text = prompt(&Job {
            title: &title,
            duration_secs: duration,
            lecture_dir: &format!("../lectures/{id}"),
            course_dir: course_dir.as_deref(),
            detected: found.len(),
            has_transcript: !cues.is_empty(),
        });

        on_step(Step::Asking);
        let chapters =
            crate::lectures::lecture_jobs::reply(job.data_dir, job.selection, &text, on_event)
                .and_then(|reply| parse_chapters(&reply))
                .and_then(|chapters| {
                    validate(&chapters, &boundaries, duration).map(|()| chapters)
                })?;
        on_step(Step::Writing);
        rt.block_on(crate::db::store::save_chapters(pool, id, &chapters))?;
        Ok(Outcome {
            title: title.clone(),
            duration_seconds: duration,
            candidates: found.len(),
            source: detected.source,
            chapters,
        })
    })();

    if let Err(e) = &outcome {
        rt.block_on(crate::db::store::set_chapter_status(
            pool,
            id,
            Some("error"),
            Some(e),
        ))?;
    }
    outcome
}

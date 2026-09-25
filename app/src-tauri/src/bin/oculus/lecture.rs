//! `oculus lecture`.

use super::*;
use app_lib::harness::{jobs, Provider};

impl Ctx {
    /// Topic-change candidates from the recording; nothing stored unless
    /// `--frames`. See `app_lib::chapters`.
    pub(crate) fn lecture_candidates(&self, args: &LectureCandidatesArgs) -> Result<(), String> {
        let pool = self.db().ok_or("lectures live in the database")?;
        let (id, title, duration, video, transcript) = self.one_lecture(&pool, &args.id)?;

        let video = video.ok_or_else(|| {
            format!("{title} is not downloaded — `oculus run -l --videos` fetches it")
        })?;
        let video = PathBuf::from(&video);
        if !video.exists() {
            return Err(format!(
                "{} is on record but missing from disk",
                video.display()
            ));
        }
        // No resource dir here: the dev copy in src-tauri/binaries, or the system's.
        let ffmpeg = app_lib::echo360::find_ffmpeg(None)
            .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;

        // Without a transcript only the pause bonus is lost.
        let gaps = transcript
            .as_deref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|vtt| app_lib::chapters::cue_gaps(&vtt))
            .unwrap_or_default();
        let dir = app_lib::echo360::lecture_dir(&self.data_dir, &id);
        // `--source` overrules the detected slide stream.
        let detected = app_lib::chapters::detect(
            &ffmpeg,
            &dir,
            &video,
            &gaps,
            duration as u32,
            args.source,
            |_| {},
        )?;
        let found = &detected.candidates;

        let frames = if args.frames {
            let seconds: Vec<u32> = found.iter().map(|c| c.seconds).collect();
            Some(app_lib::chapters::extract_frames(
                &ffmpeg,
                &detected.video,
                &seconds,
                &dir.join("frames"),
                |_| {},
            )?)
        } else {
            None
        };

        if self.json {
            #[derive(Serialize)]
            struct Out<'a> {
                lecture: &'a str,
                title: &'a str,
                duration_seconds: i64,
                source: u8,
                sampled_seconds: usize,
                candidates: &'a [app_lib::chapters::Candidate],
                #[serde(skip_serializing_if = "Option::is_none")]
                frames: Option<Vec<String>>,
            }
            return self.emit(&Out {
                lecture: &id,
                title: &title,
                duration_seconds: duration,
                source: detected.source,
                sampled_seconds: detected.diffs.len() + 1,
                candidates: found,
                frames: frames
                    .as_ref()
                    .map(|f| f.iter().map(|p| p.to_string_lossy().into_owned()).collect()),
            });
        }

        println!(
            "{}  {}  {}",
            paint(&title, BOLD),
            paint(&clock(duration as u32), DIM),
            paint(&format!("source {}", detected.source), DIM)
        );
        if found.is_empty() {
            println!("{}", paint("no boundaries — one continuous slide?", DIM));
            return Ok(());
        }
        for c in found {
            println!(
                "  {}  {}{}",
                clock(c.seconds),
                paint(&format!("{:>7.1}", c.score), DIM),
                if c.pause { paint("  pause", DIM) } else { String::new() }
            );
        }
        println!(
            "{}",
            paint(&format!("{} candidate(s)", found.len()), DIM)
        );
        if let Some(frames) = &frames {
            if let Some(first) = frames.first().and_then(|p| p.parent()) {
                println!("{}", paint(&format!("frames in {}", first.display()), DIM));
            }
        }
        Ok(())
    }

    /// Flags and printing around `chapters::run`, the job the app runs too.
    pub(crate) fn lecture_chapters(&self, args: &LectureChaptersArgs) -> Result<(), String> {
        let pool = self.db().ok_or("lectures live in the database")?;
        let (id, title, ..) = self.one_lecture(&pool, &args.id)?;

        // `chapters::run` guards this too, but cannot name the CLI's `--force`.
        if !args.force {
            let existing = self.rt.block_on(store::chapters(&pool, &id))?;
            if !existing.is_empty() {
                return Err(format!(
                    "{title} already has {} chapter(s) — `--force` re-runs and replaces them",
                    existing.len()
                ));
            }
        }

        let selection = self.job_selection(
            &pool,
            jobs::Job::LectureChapters,
            [&args.provider, &args.model, &args.effort],
        )?;
        let quiet = self.json;

        // The reply is the chapter JSON, printed below; only tool rows stream.
        let printer = AgentPrinter::new(false);
        let outcome = app_lib::chapters::run(
            self.rt.handle(),
            &pool,
            &app_lib::chapters::Run {
                data_dir: &self.data_dir,
                lecture_id: &id,
                selection: &selection,
                force: args.force,
                source: args.source,
            },
            // Of the pipeline's steps only the candidate set gets a line.
            |step| {
                if let app_lib::chapters::Step::Detected { title, duration, candidates } = step {
                    if !quiet {
                        println!(
                            "{}  {}  {}",
                            paint(title, BOLD),
                            paint(&clock(duration), DIM),
                            paint(&format!("{candidates} candidate(s)"), DIM)
                        );
                    }
                }
            },
            move |ev| {
                if !quiet {
                    printer.print(ev);
                }
            },
        )?;

        if self.json {
            #[derive(Serialize)]
            struct Out<'a> {
                lecture: &'a str,
                title: &'a str,
                duration_seconds: u32,
                provider: &'a str,
                model: &'a str,
                effort: Option<&'a str>,
                source: u8,
                candidates: usize,
                chapters: &'a [app_lib::chapters::Chapter],
            }
            return self.emit(&Out {
                lecture: &id,
                title: &outcome.title,
                duration_seconds: outcome.duration_seconds,
                provider: selection.provider.as_str(),
                model: &selection.model,
                effort: selection.effort(),
                source: outcome.source,
                candidates: outcome.candidates,
                chapters: &outcome.chapters,
            });
        }

        println!();
        for chapter in &outcome.chapters {
            println!(
                "  {}  {}",
                paint(&clock(chapter.start_seconds), DIM),
                paint(&chapter.title, BOLD)
            );
            println!("            {}", paint(&chapter.summary, DIM));
        }
        println!(
            "{}",
            paint(
                &format!(
                    "{} chapter(s) written for {}",
                    outcome.chapters.len(),
                    outcome.title
                ),
                DIM
            )
        );
        Ok(())
    }

    /// Flags and printing around `reading::run`, the job the app runs too.
    pub(crate) fn lecture_reading(&self, args: &LectureReadingArgs) -> Result<(), String> {
        let pool = self.db().ok_or("lectures live in the database")?;
        let (id, title, ..) = self.one_lecture(&pool, &args.id)?;

        // As in `lecture_chapters`: the early error can name `--force`.
        if !args.force {
            let existing = self.rt.block_on(store::reading(&pool, &id))?;
            if !existing.is_empty() {
                return Err(format!(
                    "{title} already has a reading copy of {} line(s) — `--force` re-runs and replaces it",
                    existing.len()
                ));
            }
        }

        let selection = self.job_selection(
            &pool,
            jobs::Job::LectureReading,
            [&args.provider, &args.model, &args.effort],
        )?;
        let quiet = self.json;

        // The reply is JSON that becomes the lines below; only tool rows stream.
        let printer = AgentPrinter::new(false);
        let outcome = app_lib::reading::run(
            self.rt.handle(),
            &pool,
            &app_lib::reading::Run {
                data_dir: &self.data_dir,
                lecture_id: &id,
                selection: &selection,
                force: args.force,
                source: args.source,
            },
            |step| {
                if quiet {
                    return;
                }
                match step {
                    app_lib::reading::Step::Segmented {
                        title,
                        duration,
                        segments,
                    } => println!(
                        "{}  {}  {}",
                        paint(title, BOLD),
                        paint(&clock(duration), DIM),
                        paint(&format!("{segments} segment(s)"), DIM)
                    ),
                    app_lib::reading::Step::Window {
                        done,
                        total,
                        start,
                        end,
                    } => println!(
                        "{}",
                        paint(
                            &format!(
                                "window {done} of {total} — {}–{}",
                                clock(start),
                                clock(end)
                            ),
                            DIM
                        )
                    ),
                    _ => {}
                }
            },
            move |ev| {
                if !quiet {
                    printer.print(ev);
                }
            },
        )?;

        if self.json {
            #[derive(Serialize)]
            struct Out<'a> {
                lecture: &'a str,
                title: &'a str,
                duration_seconds: u32,
                provider: &'a str,
                model: &'a str,
                effort: Option<&'a str>,
                source: u8,
                segments: usize,
                windows: usize,
                lines: &'a [app_lib::reading::ReadingLine],
            }
            return self.emit(&Out {
                lecture: &id,
                title: &outcome.title,
                duration_seconds: outcome.duration_seconds,
                provider: selection.provider.as_str(),
                model: &selection.model,
                effort: selection.effort(),
                source: outcome.source,
                segments: outcome.segments,
                windows: outcome.windows,
                lines: &outcome.lines,
            });
        }

        println!();
        for line in &outcome.lines {
            println!("  {}  {}", paint(&clock(line.start_seconds), DIM), line.text);
        }
        println!(
            "{}",
            paint(
                &format!(
                    "{} line(s) written for {}",
                    outcome.lines.len(),
                    outcome.title
                ),
                DIM
            )
        );
        Ok(())
    }

    /// The job's configured model, with `[provider, model, effort]` flags
    /// overriding what they name; the choice is printed unless `--json`.
    fn job_selection(
        &self,
        pool: &SqlitePool,
        job: jobs::Job,
        [provider, model, effort]: [&Option<String>; 3],
    ) -> Result<jobs::JobSelection, String> {
        let mut selection = self.rt.block_on(jobs::selection(pool, job));
        if let Some(p) = provider {
            selection.provider = Provider::parse(p).ok_or("unknown provider")?;
        }
        if let Some(m) = model {
            selection.model = m.clone();
        }
        if let Some(e) = effort {
            selection.reasoning_effort = Some(e.clone());
        }
        if !self.json {
            println!(
                "{}",
                paint(
                    &format!(
                        "{} · {}{}",
                        selection.provider.label(),
                        selection.model,
                        match selection.effort() {
                            Some(e) => format!(" · {e} reasoning"),
                            None => String::new(),
                        }
                    ),
                    DIM
                )
            );
        }
        Ok(selection)
    }

    /// A lecture by id or unique id prefix; an ambiguous prefix is an error.
    #[allow(clippy::type_complexity)]
    fn one_lecture(
        &self,
        pool: &SqlitePool,
        id: &str,
    ) -> Result<(String, String, i64, Option<String>, Option<String>), String> {
        let rows = self
            .rt
            .block_on(
                sqlx::query(
                    "SELECT id, title, duration_seconds, video_path, transcript_path
                     FROM lectures WHERE id = ?1 OR id LIKE ?2 ORDER BY id",
                )
                .bind(id)
                .bind(format!("{id}%"))
                .fetch_all(pool),
            )
            .map_err(|e| e.to_string())?;
        let row = match rows.len() {
            0 => {
                return Err(format!(
                    "no lecture {id} — `oculus list -l` prints the id of every lecture on record"
                ))
            }
            1 => &rows[0],
            n => {
                let found: Vec<String> = rows.iter().map(|r| r.get::<String, _>("id")).collect();
                return Err(format!(
                    "{id} matched {n} lectures ({}) — pass more of the id",
                    found.join(", ")
                ));
            }
        };
        Ok((
            row.get("id"),
            row.get("title"),
            row.get("duration_seconds"),
            row.get("video_path"),
            row.get("transcript_path"),
        ))
    }
}

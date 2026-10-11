//! `oculus studio new` and `oculus studio list`: the videos under
//! `<app data>/studio/videos/` (`app_lib::studio::videos`).

use crate::*;
use app_lib::library::paths::iso8601_utc;
use app_lib::studio::videos::{self, NewVideo, VideoRow};

impl Ctx {
    pub(crate) fn studio_new(&self, args: &StudioNewArgs) -> Result<(), String> {
        let dir = videos::create_in(
            &videos::videos_dir(),
            &NewVideo {
                slug: args.slug.clone(),
                subject: args.subject.clone(),
                title: args.title.clone(),
                portrait: args.portrait,
                ..NewVideo::default()
            },
        )?;
        if self.json {
            return self.emit(&serde_json::json!({
                "dir": dir.to_string_lossy(),
                "slug": args.slug,
            }));
        }
        println!("{} {}", paint("made", GREEN), dir.display());
        Ok(())
    }

    pub(crate) fn studio_list(&self, args: &StudioListArgs) -> Result<(), String> {
        let rows = videos::list_in(&videos::videos_dir(), args.subject.as_deref())?;
        if self.json {
            let rows: Vec<serde_json::Value> = rows.iter().map(row_json).collect();
            return self.emit(&rows);
        }
        if rows.is_empty() {
            println!(
                "{}",
                paint(
                    &format!(
                        "no videos in {}; `oculus studio new <slug>` makes one",
                        videos::videos_dir().display()
                    ),
                    DIM
                )
            );
        }
        for row in &rows {
            print_row(row);
        }
        Ok(())
    }
}

fn row_json(row: &VideoRow) -> serde_json::Value {
    serde_json::json!({
        "slug": row.slug,
        "dir": row.dir.to_string_lossy(),
        "subject": row.subject,
        "scenes": row.scenes,
        "narrated": row.narrated,
        "renders": row.renders,
        "modified": row.modified.map(iso8601_utc),
        "problem": row.problem,
    })
}

/// `slug  SUBJECT  3 scenes  narrated  draft.mp4  2026-10-12T03:14:00`, or
/// the slug and why its video.json does not load.
fn print_row(row: &VideoRow) {
    if let Some(problem) = &row.problem {
        println!(
            "{} {}",
            paint(&format!("{:<28}", row.slug), BOLD),
            paint(problem, YELLOW)
        );
        return;
    }
    let scenes = row.scenes.unwrap_or(0);
    let renders = if row.renders.is_empty() {
        "no renders".to_owned()
    } else {
        row.renders.join(", ")
    };
    println!(
        "{} {:<12} {:>2} scene{} {:<10} {:<22} {}",
        paint(&format!("{:<28}", row.slug), BOLD),
        row.subject.as_deref().unwrap_or("-"),
        scenes,
        if scenes == 1 { " " } else { "s" },
        if row.narrated {
            "narrated"
        } else {
            "unnarrated"
        },
        renders,
        paint(&row.modified.map(iso8601_utc).unwrap_or_default(), DIM),
    );
}

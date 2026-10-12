//! `oculus studio gallery`: one PNG per example of a component
//! (`studio_core::gallery`, docs/studio.md, "Galleries render every example
//! as a still"). It never reads the narration, so it runs before `narrate`.

use super::studio_error;
use crate::*;
use app_lib::studio::libraries;
use std::time::Instant;
use studio_core::gallery::{self, GalleryOptions};

impl Ctx {
    pub(crate) fn studio_gallery(&self, args: &StudioGalleryArgs) -> Result<(), String> {
        let project =
            libraries::open_unnarrated(&args.dir, args.subject.clone(), args.theme.clone())
                .map_err(studio_error)?;
        let options = GalleryOptions {
            scale: 1.0,
            threads: args.threads,
        };
        let started = Instant::now();
        let gallery =
            gallery::render(&project, &args.component, args.t, &options).map_err(studio_error)?;
        let out = args
            .out
            .clone()
            .unwrap_or_else(|| args.dir.join("out").join("gallery"));
        std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        let mut stills = Vec::new();
        for still in &gallery.stills {
            let path = out.join(format!("{}-{}.png", args.component, still.example));
            still.frame.write_png(&path).map_err(studio_error)?;
            stills.push((
                still.example.clone(),
                path,
                still.frame.width,
                still.frame.height,
            ));
        }
        let sheet = if args.sheet {
            let path = out.join(format!("{}.png", args.component));
            gallery
                .write_sheet(&path, args.cols.max(1))
                .map_err(studio_error)?;
            Some(path)
        } else {
            None
        };
        let ms = started.elapsed().as_millis();
        if self.json {
            return self.emit(&serde_json::json!({
                "component": args.component,
                "t": gallery.moment.t,
                "duration": gallery.moment.duration,
                "threads": gallery.threads,
                "ms": ms,
                "stills": stills.iter().map(|(example, path, w, h)| serde_json::json!({
                    "example": example,
                    "path": path.to_string_lossy(),
                    "width": w,
                    "height": h,
                })).collect::<Vec<_>>(),
                "sheet": sheet.as_ref().map(|p| p.to_string_lossy().into_owned()),
            }));
        }
        for (example, path, w, h) in &stills {
            println!(
                "{} {} ({example}, {w}×{h})",
                paint("wrote", GREEN),
                path.display()
            );
        }
        if let Some(path) = &sheet {
            println!("{} {}", paint("wrote", GREEN), path.display());
        }
        println!(
            "{}",
            paint(
                &format!(
                    "  {} at {:.3} s of {:.3} s · {} example(s) · {} thread(s) · {ms} ms",
                    args.component,
                    gallery.moment.t,
                    gallery.moment.duration,
                    gallery.stills.len(),
                    gallery.threads
                ),
                DIM
            )
        );
        Ok(())
    }
}

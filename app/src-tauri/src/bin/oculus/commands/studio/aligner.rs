//! `oculus studio aligner`: the narration aligner model in
//! `<app data>/models/align/` (`app_lib::studio::timing`).

use crate::*;
use app_lib::studio::timing::{self, Status, MODEL_ENV, MODEL_ID, PACK};
use std::time::{Duration, Instant};

impl Ctx {
    pub(crate) fn studio_aligner(&self, action: &StudioAlignerAction) -> Result<(), String> {
        let dir = timing::dir();
        match action {
            StudioAlignerAction::Status => self.aligner_status(&dir),
            StudioAlignerAction::Download => self.aligner_download(&dir),
            StudioAlignerAction::Delete => {
                let freed = timing::delete(&dir)?;
                if self.json {
                    return self.emit(&serde_json::json!({ "deleted_bytes": freed }));
                }
                match freed {
                    0 => println!("no aligner model in {}", dir.display()),
                    n => println!(
                        "{} the aligner model ({})",
                        paint("deleted", GREEN),
                        human_bytes(n)
                    ),
                }
                Ok(())
            }
        }
    }

    fn aligner_status(&self, dir: &std::path::Path) -> Result<(), String> {
        let status = timing::status(dir);
        let path = dir.join(PACK.files[0].file);
        let overridden = std::env::var_os(MODEL_ENV).filter(|v| !v.is_empty());
        if self.json {
            return self.emit(&serde_json::json!({
                "status": status,
                "model": MODEL_ID,
                "path": path.to_string_lossy(),
                "bytes": PACK.bytes(),
                "override": overridden.as_ref().map(|v| v.to_string_lossy()),
            }));
        }
        let word = match status {
            Status::Present => paint("downloaded", GREEN),
            Status::Downloading => paint("downloading", YELLOW),
            Status::Missing => paint("not downloaded", YELLOW),
        };
        println!("{word} {MODEL_ID} ({})", human_bytes(PACK.bytes()));
        println!("{}", paint(&format!("  {}", path.display()), DIM));
        if let Some(v) = overridden {
            println!(
                "{}",
                paint(
                    &format!("  {MODEL_ENV} is set: narrate uses {}", v.to_string_lossy()),
                    DIM
                )
            );
        } else if status == Status::Missing {
            println!(
                "{}",
                paint("  `oculus studio aligner download` fetches it", DIM)
            );
        }
        Ok(())
    }

    fn aligner_download(&self, dir: &std::path::Path) -> Result<(), String> {
        if timing::status(dir) == Status::Present {
            if self.json {
                return self.emit(&serde_json::json!({ "status": Status::Present }));
            }
            println!("already downloaded: {}", dir.display());
            return Ok(());
        }
        let quiet = self.json;
        let last = std::sync::Mutex::new((Instant::now(), u64::MAX));
        let started = Instant::now();
        timing::download(
            dir,
            |received, total| {
                if quiet || total == 0 {
                    return;
                }
                let mut last = last.lock().unwrap();
                let percent = received * 100 / total;
                let due = last.0.elapsed() >= Duration::from_secs(2);
                if percent != last.1 && (due || received == total || last.1 == u64::MAX) {
                    *last = (Instant::now(), percent);
                    println!(
                        "{}",
                        paint(
                            &format!(
                                "  {percent:>3}%  {} of {}",
                                human_bytes(received),
                                human_bytes(total)
                            ),
                            DIM
                        )
                    );
                }
            },
            || false,
        )?;
        if self.json {
            return self.emit(&serde_json::json!({
                "status": Status::Present,
                "path": dir.join(PACK.files[0].file).to_string_lossy(),
                "seconds": started.elapsed().as_secs_f64(),
            }));
        }
        println!(
            "{} the aligner model to {} in {:.0} s",
            paint("downloaded", GREEN),
            dir.display(),
            started.elapsed().as_secs_f64()
        );
        Ok(())
    }
}

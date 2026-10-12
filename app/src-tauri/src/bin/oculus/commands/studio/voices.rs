//! `oculus studio voices`: Kokoro's two packs in `<app data>/models/voice/`
//! and the voices in them (`app_lib::studio::tts::kokoro`).

use crate::*;
use app_lib::runtime::onnx::{self, Status};
use app_lib::studio::tts::kokoro;
use app_lib::studio::voice::{self, Engine};
use std::time::{Duration, Instant};

impl Ctx {
    pub(crate) fn studio_voices(&self, action: &Option<StudioVoicesAction>) -> Result<(), String> {
        let dir = kokoro::dir();
        match action {
            None | Some(StudioVoicesAction::List) => self.voices_list(&dir),
            Some(StudioVoicesAction::Download) => self.voices_download(&dir),
            Some(StudioVoicesAction::Delete) => {
                let freed = kokoro::delete(&dir)?;
                if self.json {
                    return self.emit(&serde_json::json!({ "deleted_bytes": freed }));
                }
                match freed {
                    0 => println!("no Kokoro packs in {}", dir.display()),
                    n => println!(
                        "{} Kokoro's packs ({})",
                        paint("deleted", GREEN),
                        human_bytes(n)
                    ),
                }
                Ok(())
            }
        }
    }

    fn voices_list(&self, dir: &std::path::Path) -> Result<(), String> {
        let settings = voice::settings();
        let chosen = (settings.engine == Some(Engine::Kokoro))
            .then_some(settings.voice)
            .flatten();
        let packs: Vec<_> = kokoro::packs()
            .iter()
            .map(|p| (p.id, onnx::status(dir, p), p.bytes()))
            .collect();
        let voices = kokoro::voices(dir);
        if self.json {
            return self.emit(&serde_json::json!({
                "dir": dir.to_string_lossy(),
                "packs": packs.iter().map(|(id, status, bytes)| serde_json::json!({
                    "id": id, "status": status, "bytes": bytes,
                })).collect::<Vec<_>>(),
                "voices": voices.iter().map(|(id, present)| serde_json::json!({
                    "id": id,
                    "downloaded": present,
                    "chosen": chosen.as_deref() == Some(id.as_str()),
                })).collect::<Vec<_>>(),
            }));
        }
        for (id, status, bytes) in &packs {
            println!("{} {id} ({})", status_word(*status), human_bytes(*bytes));
        }
        println!("{}", paint(&format!("  {}", dir.display()), DIM));
        println!();
        for (id, present) in &voices {
            let state = if *present {
                paint("downloaded", GREEN)
            } else {
                paint("not downloaded", YELLOW)
            };
            let mark = if chosen.as_deref() == Some(id.as_str()) {
                paint("  (Settings -> Studio -> Narration)", DIM)
            } else {
                String::new()
            };
            println!("{} {state}{mark}", paint(&format!("{id:<12}"), BOLD));
        }
        if packs.iter().any(|(_, s, _)| *s == Status::Missing) {
            println!(
                "{}",
                paint("  `oculus studio voices download` fetches both packs", DIM)
            );
        }
        Ok(())
    }

    fn voices_download(&self, dir: &std::path::Path) -> Result<(), String> {
        if kokoro::packs().iter().all(|p| onnx::is_present(dir, p)) {
            if self.json {
                return self.emit(&serde_json::json!({ "status": Status::Present }));
            }
            println!("already downloaded: {}", dir.display());
            return Ok(());
        }
        let quiet = self.json;
        let last = std::sync::Mutex::new((Instant::now(), u64::MAX));
        let started = Instant::now();
        kokoro::download(
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
                "dir": dir.to_string_lossy(),
                "seconds": started.elapsed().as_secs_f64(),
            }));
        }
        println!(
            "{} Kokoro's packs to {} in {:.0} s",
            paint("downloaded", GREEN),
            dir.display(),
            started.elapsed().as_secs_f64()
        );
        Ok(())
    }
}

fn status_word(status: Status) -> String {
    match status {
        Status::Present => paint("downloaded", GREEN),
        Status::Downloading => paint("downloading", YELLOW),
        Status::Missing => paint("not downloaded", YELLOW),
    }
}

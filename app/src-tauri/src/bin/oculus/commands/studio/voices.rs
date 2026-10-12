//! `oculus studio voices`: Kokoro's model and pronunciation packs and its 20
//! voices in `<app data>/models/voice/` (`app_lib::studio::tts::kokoro`).

use crate::*;
use app_lib::runtime::onnx::{self, Status};
use app_lib::studio::tts::kokoro::{self, voices, voices::Voice};
use app_lib::studio::voice::{self, Engine};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The voices a bare `download` fetches with the packs: Settings'
/// suggestions on a fresh install.
const STARTERS: [&str; 2] = ["af_heart", "am_michael"];

impl Ctx {
    pub(crate) fn studio_voices(&self, action: &Option<StudioVoicesAction>) -> Result<(), String> {
        let dir = kokoro::dir();
        match action {
            None | Some(StudioVoicesAction::List) => self.voices_list(&dir),
            Some(StudioVoicesAction::Download(args)) => self.voices_download(&dir, args),
            Some(StudioVoicesAction::Delete(args)) => self.voices_delete(&dir, args),
        }
    }

    fn voices_list(&self, dir: &Path) -> Result<(), String> {
        let settings = voice::settings();
        let chosen = (settings.engine == Some(Engine::Kokoro))
            .then_some(settings.voice)
            .flatten();
        let is_chosen = |id: &str| chosen.as_deref() == Some(id);
        let packs: Vec<_> = kokoro::packs()
            .iter()
            .map(|p| (p.id, onnx::status(dir, p), p.bytes()))
            .collect();
        if self.json {
            return self.emit(&serde_json::json!({
                "dir": dir.to_string_lossy(),
                "packs": packs.iter().map(|(id, status, bytes)| serde_json::json!({
                    "id": id, "status": status, "bytes": bytes,
                })).collect::<Vec<_>>(),
                "voices": voices::VOICES.iter().map(|v| serde_json::json!({
                    "id": v.id,
                    "name": v.name,
                    "accent": v.accent.label(),
                    "gender": v.gender.label(),
                    "grade": v.grade,
                    "bytes": v.bytes(),
                    "status": voices::status(dir, v),
                    "chosen": is_chosen(v.id),
                })).collect::<Vec<_>>(),
            }));
        }
        for (id, status, bytes) in &packs {
            println!("{} {id} ({})", status_word(*status), human_bytes(*bytes));
        }
        println!("{}", paint(&format!("  {}", dir.display()), DIM));
        println!();
        for v in &voices::VOICES {
            let mark = if is_chosen(v.id) {
                paint("  (Settings -> Studio -> Narration)", DIM)
            } else {
                String::new()
            };
            println!(
                "{} {:<15} {:<2}  {}{mark}",
                paint(&format!("{:<11}", v.id), BOLD),
                format!("{} {}", v.accent.label(), v.gender.label()),
                v.grade,
                status_word(voices::status(dir, v)),
            );
        }
        if packs.iter().any(|(_, s, _)| *s == Status::Missing) {
            println!(
                "{}",
                paint(
                    "  `oculus studio voices download` fetches the packs and the starter voices",
                    DIM
                )
            );
        }
        Ok(())
    }

    fn voices_download(&self, dir: &Path, args: &StudioVoicesDownloadArgs) -> Result<(), String> {
        let named = picked(&args.ids, args.all)?;
        let fetch_packs = named.is_none();
        let wanted: Vec<&'static Voice> = match named {
            Some(v) => v,
            None => STARTERS
                .iter()
                .map(|id| voices::find(id).ok_or_else(|| format!("no voice `{id}`")))
                .collect::<Result<_, _>>()?,
        };
        let started = Instant::now();
        let mut fetched: Vec<&str> = Vec::new();
        if fetch_packs && !kokoro::packs().iter().all(|p| onnx::is_present(dir, p)) {
            kokoro::download(dir, self.progress_printer("the packs"), || false)?;
            fetched.extend(kokoro::packs().iter().map(|p| p.id));
        }
        for v in wanted {
            if voices::status(dir, v) == Status::Present {
                continue;
            }
            voices::download(dir, v, self.progress_printer(v.id), || false)?;
            fetched.push(v.id);
        }
        if self.json {
            return self.emit(&serde_json::json!({
                "dir": dir.to_string_lossy(),
                "downloaded": fetched,
                "seconds": started.elapsed().as_secs_f64(),
            }));
        }
        if fetched.is_empty() {
            println!("already downloaded: {}", dir.display());
        } else {
            println!(
                "{} {} to {} in {:.0} s",
                paint("downloaded", GREEN),
                fetched.join(", "),
                dir.display(),
                started.elapsed().as_secs_f64()
            );
        }
        Ok(())
    }

    fn voices_delete(&self, dir: &Path, args: &StudioVoicesDeleteArgs) -> Result<(), String> {
        let (what, freed) = match picked(&args.ids, args.all)? {
            None => ("the packs".to_owned(), kokoro::delete(dir)?),
            Some(named) => {
                let mut freed = 0;
                for v in &named {
                    freed += voices::delete(dir, v)?;
                }
                let ids: Vec<_> = named.iter().map(|v| v.id).collect();
                (ids.join(", "), freed)
            }
        };
        if self.json {
            return self.emit(&serde_json::json!({ "deleted_bytes": freed }));
        }
        match freed {
            0 => println!("nothing of {what} in {}", dir.display()),
            n => println!("{} {what} ({})", paint("deleted", GREEN), human_bytes(n)),
        }
        Ok(())
    }

    /// A progress callback printing `label`'s percentage about every two
    /// seconds, for the first and last update too; silent under `--json`.
    fn progress_printer(&self, label: &str) -> impl Fn(u64, u64) {
        let quiet = self.json;
        let label = label.to_owned();
        let last = Mutex::new((Instant::now(), u64::MAX));
        move |received, total| {
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
                            "  {label}: {percent:>3}%  {} of {}",
                            human_bytes(received),
                            human_bytes(total)
                        ),
                        DIM
                    )
                );
            }
        }
    }
}

/// The voices `ids` name, or all of them for `--all`; `None` when neither
/// was given. An id outside the catalogue fails, naming it.
fn picked(ids: &[String], all: bool) -> Result<Option<Vec<&'static Voice>>, String> {
    if all {
        return Ok(Some(voices::VOICES.iter().collect()));
    }
    if ids.is_empty() {
        return Ok(None);
    }
    ids.iter()
        .map(|id| {
            voices::find(id).ok_or_else(|| {
                format!("`{id}` is not one of Kokoro's voices; `oculus studio voices` lists them")
            })
        })
        .collect::<Result<_, _>>()
        .map(Some)
}

fn status_word(status: Status) -> String {
    match status {
        Status::Present => paint("downloaded", GREEN),
        Status::Downloading => paint("downloading", YELLOW),
        Status::Missing => paint("not downloaded", YELLOW),
    }
}

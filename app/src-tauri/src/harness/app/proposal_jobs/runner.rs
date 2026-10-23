use super::{strings, text};
use crate::harness::app::HarnessState;
use serde_json::Value;
use sqlx::Row;
use tauri::{AppHandle, Listener, Manager};
async fn lecture(app: &AppHandle, kind: &str, id: String) -> Result<(), String> {
    let event = if kind == "LectureChapters" {
        "lecture-chapters"
    } else {
        "lecture-end"
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = std::sync::Mutex::new(Some(sender));
    let expected = id.clone();
    let listener = app.listen(event, move |event| {
        let Ok(payload) = serde_json::from_str::<Value>(event.payload()) else {
            return;
        };
        if payload.get("lectureId").and_then(Value::as_str) != Some(&expected) {
            return;
        }
        if let Some(sender) = sender.lock().unwrap().take() {
            let result = if payload.get("status").and_then(Value::as_str) == Some("error") {
                Err(payload
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("lecture job failed")
                    .to_owned())
            } else {
                Ok(())
            };
            let _ = sender.send(result);
        }
    });
    let started = if kind == "LectureChapters" {
        crate::lectures::chapters::app::lecture_find_chapters(app.clone(), id, None, None).await
    } else {
        crate::lectures::lecture_end::app::lecture_find_end(
            app.clone(),
            app.state::<HarnessState>(),
            id,
            None,
        )
        .await
    };
    let result = match started {
        Err(e) => Err(e),
        Ok(()) => receiver.await.map_err(|e| e.to_string()).and_then(|v| v),
    };
    app.unlisten(listener);
    result
}

pub(super) async fn run(app: &AppHandle, kind: &str, state: &Value) -> Result<(), String> {
    match kind {
        "Transcribe" => {
            crate::transcribe::app::transcribe_video(app.clone(), text(state, "file")?.into())
                .await
                .map(|_| ())
        }
        "LectureChapters" | "LectureEnd" => {
            lecture(app, kind, text(state, "lecture")?.into()).await
        }
        "Download" => match text(state, "item")? {
            "aligner" => {
                crate::studio::commands::aligner::studio_aligner_download(app.clone()).await
            }
            "voices" => {
                crate::studio::commands::kokoro::studio_voice_packs_download(app.clone()).await?;
                for id in ["af_heart", "am_michael"] {
                    crate::studio::commands::voice_files::studio_voice_download(
                        app.clone(),
                        id.into(),
                    )
                    .await?;
                }
                Ok(())
            }
            id => {
                crate::studio::commands::voice_files::studio_voice_download(app.clone(), id.into())
                    .await
            }
        },
        "Reparse" => reparse(strings(state, "files")?).await,
        "Sync" => {
            sync(
                app.clone(),
                strings(state, "subjects")?,
                state
                    .get("lectures")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )
            .await
        }
        _ => Err("unknown job".into()),
    }
}

async fn reparse(files: Vec<String>) -> Result<(), String> {
    let pool = crate::db::store::open_pool().await?;
    for rel in files {
        let row = sqlx::query("SELECT id,subject_id FROM files WHERE relative_path=?1")
            .bind(&rel)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("no library file {rel}"))?;
        let subject: i64 = row.get("subject_id");
        let file: i64 = row.get("id");
        let parse_rel = rel.clone();
        crate::runtime::blocking::run(move || {
            crate::sync::parse_pdf_reporting(
                &crate::library::paths::data_dir(),
                &parse_rel,
                subject,
                true,
                &|_| {},
            )
            .map_err(|e| e.to_string())
        })
        .await?;
        crate::pages::retrieval::commands::embed_file(file, subject, rel, Some(false)).await?;
    }
    Ok(())
}

async fn sync(app: AppHandle, codes: Vec<String>, lectures: bool) -> Result<(), String> {
    let pool = crate::db::store::open_pool().await?;
    let subjects = crate::db::store::subjects(&pool).await?;
    let mut targets = Vec::new();
    for code in codes {
        let matches: Vec<_> = subjects
            .iter()
            .filter(|s| {
                s.code.eq_ignore_ascii_case(&code)
                    || s.code
                        .to_ascii_uppercase()
                        .starts_with(&format!("{}_", code.to_ascii_uppercase()))
            })
            .collect();
        let current: Vec<_> = matches.iter().filter(|s| s.is_current).collect();
        let selected = if current.len() == 1 {
            current[0]
        } else if matches.len() == 1 {
            matches[0]
        } else {
            return Err(format!("subject {code} is missing or ambiguous"));
        };
        targets.push(crate::sync::Subject {
            id: selected.id,
            code: selected.code.clone(),
        });
    }
    crate::sync::scrape::confirmed(app, targets, lectures).await
}

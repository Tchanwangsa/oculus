use super::{strings, text};
use crate::harness::app::HarnessState;
use serde_json::Value;
use sqlx::Row;
use tauri::{AppHandle, Listener, Manager};
async fn lecture(app: &AppHandle, kind: &str, id: String) -> Result<String, String> {
    let event = if kind == "LectureChapters" {
        "lecture-chapters"
    } else {
        "lecture-end"
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = std::sync::Mutex::new(Some(sender));
    let expected = id.clone();
    let kind_is_chapters = kind == "LectureChapters";
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
                Ok(if kind_is_chapters {
                    format!(
                        "{} chapters found",
                        payload["chapters"].as_u64().unwrap_or(0)
                    )
                } else if let Some(seconds) = payload["seconds"].as_u64() {
                    format!(
                        "Lecture content ends at {}:{:02}",
                        seconds / 60,
                        seconds % 60
                    )
                } else {
                    "No lecture end found".into()
                })
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

pub(super) async fn run(
    app: &AppHandle,
    proposal_id: &str,
    kind: &str,
    state: &Value,
) -> Result<String, String> {
    match kind {
        "Transcribe" => {
            crate::transcribe::app::transcribe_video(app.clone(), text(state, "file")?.into())
                .await
                .map(|_| "Transcript saved".into())
        }
        "LectureChapters" | "LectureEnd" => {
            lecture(app, kind, text(state, "lecture")?.into()).await
        }
        "Download" => {
            let item = text(state, "item")?;
            match item {
                "aligner" => {
                    crate::studio::commands::aligner::studio_aligner_download(app.clone()).await
                }
                "voices" => {
                    crate::studio::commands::kokoro::studio_voice_packs_download(app.clone())
                        .await?;
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
                    crate::studio::commands::voice_files::studio_voice_download(
                        app.clone(),
                        id.into(),
                    )
                    .await
                }
            }
            .map(|()| format!("{item} downloaded"))
        }
        "Reparse" => reparse(strings(state, "files")?).await,
        "Sync" => {
            sync(
                app.clone(),
                proposal_id,
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

async fn reparse(files: Vec<String>) -> Result<String, String> {
    let pool = crate::db::store::open_pool().await?;
    let count = files.len();
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
    Ok(format!("{count} files parsed and embedded"))
}

async fn sync(
    app: AppHandle,
    proposal_id: &str,
    codes: Vec<String>,
    lectures: bool,
) -> Result<String, String> {
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
    let counts = std::sync::Arc::new(std::sync::Mutex::new((0_u64, 0_u64)));
    let mut listener = None;
    let app_ref = &app;
    let counts_ref = &counts;
    let listener_ref = &mut listener;
    let result =
        crate::sync::scrape::confirmed(app.clone(), targets, lectures, |run_id| async move {
            let app = app_ref;
            super::lifecycle::record_run(app, proposal_id, run_id).await?;
            let counts = counts_ref.clone();
            *listener_ref = Some(app.listen("scrape-file", move |event| {
                let Ok(file) = serde_json::from_str::<Value>(event.payload()) else {
                    return;
                };
                if file["action"] != "new" {
                    return;
                }
                let mut counts = counts.lock().unwrap();
                if file["category"] == "ed" {
                    counts.1 += 1;
                } else {
                    counts.0 += 1;
                }
            }));
            Ok(())
        })
        .await;
    if let Some(listener) = listener {
        app.unlisten(listener);
    }
    result?;
    let (files, threads) = *counts.lock().unwrap();
    Ok(if lectures {
        "Lecture lists refreshed".into()
    } else {
        format!("{files} new files, {threads} new Ed threads")
    })
}

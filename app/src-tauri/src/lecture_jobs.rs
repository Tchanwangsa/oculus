//! Inputs, one-turn agent replies and app orchestration shared by the chapter
//! and reading-copy jobs. Each job owns its algorithm, claim rules and commit
//! boundary.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use sqlx::Row;

use crate::harness::{self, HarnessEvent};

pub struct Run<'a> {
    pub data_dir: &'a Path,
    /// A full lecture id (prefix matching is the CLI's).
    pub lecture_id: &'a str,
    /// Already resolved by the caller (`harness::jobs`).
    pub selection: &'a crate::harness::jobs::JobSelection,
    /// Replace an existing derived result instead of refusing.
    pub force: bool,
    /// Read this stream instead of letting `chapters::detect` choose.
    pub source: Option<crate::echo360::SourceNum>,
}

pub(crate) struct Source {
    pub(crate) title: String,
    pub(crate) duration: u32,
    pub(crate) video: Option<String>,
    pub(crate) transcript: Option<String>,
    pub(crate) code: Option<String>,
}

impl Source {
    /// Called after each job's existing-result guard, so refusing a regeneration
    /// keeps its own message even when the recording has since been deleted.
    pub(crate) fn video(&self) -> Result<PathBuf, String> {
        let path = self.video.as_deref().ok_or_else(|| format!(
            "{} is not downloaded — `oculus run -l --videos` fetches it", self.title
        ))?;
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(format!("{} is on record but missing from disk", path.display()));
        }
        Ok(path)
    }
}

pub(crate) async fn source(pool: &sqlx::SqlitePool, id: &str) -> Result<Source, String> {
    let row = sqlx::query(
        "SELECT l.title, l.duration_seconds, l.video_path, l.transcript_path, s.code
           FROM lectures l LEFT JOIN subjects s ON s.id = l.subject_id
          WHERE l.id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("no lecture {id}"))?;
    Ok(Source {
        title: row.get("title"),
        duration: row.get::<i64, _>("duration_seconds").max(0) as u32,
        video: row.get("video_path"),
        transcript: row.get("transcript_path"),
        code: row.get("code"),
    })
}

/// One agent turn on the job's selected model, returning the assistant's text.
/// `on_event` still sees every event, so a caller can show progress.
pub(crate) fn reply(
    data_dir: &Path,
    selection: &harness::jobs::JobSelection,
    prompt: &str,
    on_event: impl Fn(&HarnessEvent) + Send + Sync + 'static,
) -> Result<String, String> {
    let reply = Arc::new(Mutex::new(String::new()));
    let collect = reply.clone();
    let options = harness::SendOptions {
        model: Some(selection.model.clone()),
        reasoning_effort: selection.reasoning_effort.clone(),
        ..Default::default()
    };
    harness::run_once(data_dir, selection.provider, &options, prompt, move |event| {
        if let HarnessEvent::AssistantMessage { text } = event {
            collect.lock().unwrap().push_str(text);
        }
        on_event(event);
    })?;
    let text = reply.lock().unwrap().clone();
    Ok(text)
}

/// A reading-copy window count; chaptering never sets it.
#[derive(serde::Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowProgress {
    pub(crate) done: u32,
    pub(crate) total: u32,
}

/// One step of a lecture job in flight, shared by chapters and `reading`.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Progress {
    pub(crate) lecture_id: String,
    /// `decoding` | `frames` | `agent` | `naming` | `writing`.
    pub(crate) phase: &'static str,
    /// A tool's own title while the agent works.
    pub(crate) detail: Option<String>,
    pub(crate) kind: Option<crate::harness::ToolKind>,
    /// Countable phases only; the agent turn has no denominator.
    pub(crate) done: Option<u32>,
    pub(crate) total: Option<u32>,
    pub(crate) window: Option<WindowProgress>,
}

impl Progress {
    pub(crate) fn at(lecture_id: &str, phase: &'static str) -> Progress {
        Progress {
            lecture_id: lecture_id.to_string(),
            phase,
            detail: None,
            kind: None,
            done: None,
            total: None,
            window: None,
        }
    }
}

/// Refuse a `source` that is not 1 or 2, or a lecture whose
/// `status_column` says a run is already in flight.
pub(crate) async fn check_start(
    lecture_id: &str,
    source: Option<u8>,
    status_column: &str,
    busy: &str,
) -> Result<(), String> {
    if let Some(n) = source {
        if n != 1 && n != 2 {
            return Err(format!("{n} is not a source — a capture has 1 and sometimes 2"));
        }
    }
    let pool = crate::store::open_pool().await?;
    let running: Option<String> =
        sqlx::query_scalar(&format!("SELECT {status_column} FROM lectures WHERE id = ?1"))
            .bind(lecture_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?
            .flatten();
    if running.as_deref() == Some("running") {
        return Err(busy.into());
    }
    Ok(())
}

/// Run `body` on a thread of its own with its own runtime, pool and the
/// agent selection configured for `job`. `tag` prefixes setup errors.
pub(crate) fn spawn_job(
    tag: &'static str,
    job: crate::harness::jobs::Job,
    body: impl FnOnce(&tokio::runtime::Runtime, &sqlx::SqlitePool, crate::harness::jobs::JobSelection)
        + Send
        + 'static,
) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => return eprintln!("[oculus] {tag}: {e}"),
        };
        let pool = match rt.block_on(crate::store::open_pool()) {
            Ok(p) => p,
            Err(e) => return eprintln!("[oculus] {tag}: {e}"),
        };
        let selection = rt.block_on(crate::harness::jobs::selection(&pool, job));
        body(&rt, &pool, selection);
    });
}

/// Startup sweep: clear a `running` status a killed run left behind.
pub(crate) fn reconcile_status<F>(
    tag: &'static str,
    sweep: impl FnOnce(sqlx::SqlitePool) -> F + Send + 'static,
) where
    F: std::future::Future<Output = Result<u64, String>> + Send,
{
    tauri::async_runtime::spawn(async move {
        if let Ok(pool) = crate::store::open_pool().await {
            if let Ok(n) = sweep(pool).await {
                if n > 0 {
                    eprintln!("[oculus] {tag}: cleared {n} interrupted run(s)");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lecture_sources_keep_optional_fields_and_reject_unknown_ids() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE subjects (id INTEGER PRIMARY KEY, code TEXT)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE lectures (id TEXT PRIMARY KEY, title TEXT, duration_seconds INTEGER,
                     video_path TEXT, transcript_path TEXT, subject_id INTEGER)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO lectures VALUES ('one', 'Lecture one', -1, NULL, NULL, NULL)")
            .execute(&pool).await.unwrap();
        let lecture = source(&pool, "one").await.unwrap();
        assert_eq!(lecture.title, "Lecture one");
        assert_eq!(lecture.duration, 0);
        assert!(lecture.transcript.is_none());
        assert!(lecture.code.is_none());
        assert!(lecture.video().unwrap_err().contains("not downloaded"));
        assert!(matches!(source(&pool, "missing").await, Err(error) if error == "no lecture missing"));
    }
}

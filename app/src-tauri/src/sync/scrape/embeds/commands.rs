//! The embed card's commands: resolve a link, download it, cancel that.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};

use super::naming::video_rel;
use super::resolve::{properties_for, save_poster, Resolved};
use super::row::{current, EmbedRow};
use super::state::{EmbedBearer, EmbedCancels};
use crate::db::store::{
    echo_embed, echo_embed_file_of_media, open_pool, set_echo_embed_file, upsert_echo_embed,
    EchoEmbed,
};
use crate::library::paths;
use crate::sources::echo360::{self, embed};
use crate::sources::institution;
use crate::sync::FileEvent;

pub const RESTRICTED: &str = "Download is turned off for this video";

/// A link's card: the cached row, or one resolved now (launch, Bearer,
/// properties, poster) and cached for good, since a link's media and secure
/// link never change.
#[tauri::command]
pub async fn echo360_embed_info(
    bearer: tauri::State<'_, EmbedBearer>,
    subject_id: i64,
    subject_code: String,
    link_id: String,
    src: String,
) -> Result<EmbedRow, String> {
    let retrieve = embed::retrieve_url(&institution::canvas_base()?, &src, &link_id)?;
    let data_dir = paths::data_dir();
    let pool = open_pool().await?;
    let row = cached_or_resolved(
        &pool,
        bearer.inner(),
        &data_dir,
        subject_id,
        &subject_code,
        &link_id,
        &retrieve,
    )
    .await?;
    current(&pool, &data_dir, row).await
}

/// Download a link's video into the course's `files/`, with its captions
/// beside it. The `files` row arrives as a `scrape-file` event, like a synced
/// file; progress as `echo-embed-progress`.
#[tauri::command]
pub async fn echo360_embed_download(
    app: AppHandle,
    bearer: tauri::State<'_, EmbedBearer>,
    cancels: tauri::State<'_, EmbedCancels>,
    subject_id: i64,
    subject_code: String,
    link_id: String,
    src: String,
) -> Result<EmbedRow, String> {
    let retrieve = embed::retrieve_url(&institution::canvas_base()?, &src, &link_id)?;
    let guard = cancels.register(&link_id)?;
    let job = Download {
        app: app.clone(),
        bearer: bearer.inner().clone(),
        data_dir: paths::data_dir(),
        subject_id,
        code: subject_code,
        link_id: link_id.clone(),
        retrieve,
    };
    let result = job.run(&guard.flag).await;
    drop(guard);
    match &result {
        Ok(_) => emit(&app, &link_id, 100, "complete"),
        Err(e) if e == crate::runtime::stream::CANCELLED => emit(&app, &link_id, 0, "cancelled"),
        Err(e) => {
            elog!("[oculus] echo embed {link_id}: {e}");
            emit(&app, &link_id, 0, "error");
        }
    }
    result
}

/// Ask an in-flight embed download to stop; false when none runs for that link.
#[tauri::command]
pub fn echo360_embed_cancel(cancels: tauri::State<'_, EmbedCancels>, link_id: String) -> bool {
    match cancels.0.lock().unwrap().get(&link_id) {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

fn emit(app: &AppHandle, link_id: &str, percent: u8, phase: &str) {
    app.emit(
        "echo-embed-progress",
        serde_json::json!({ "linkId": link_id, "percent": percent, "phase": phase }),
    )
    .ok();
}

async fn cached_or_resolved(
    pool: &SqlitePool,
    bearer: &EmbedBearer,
    data_dir: &Path,
    subject_id: i64,
    code: &str,
    link_id: &str,
    retrieve: &str,
) -> Result<EchoEmbed, String> {
    if let Some(row) = echo_embed(pool, link_id).await? {
        return Ok(row);
    }
    let (bearer, data_dir, code, link, retrieve) = (
        bearer.clone(),
        data_dir.to_path_buf(),
        code.to_string(),
        link_id.to_string(),
        retrieve.to_string(),
    );
    let record = crate::runtime::blocking::run(move || {
        let resolved = properties_for(&data_dir, &retrieve, None, &bearer)?;
        let poster = save_poster(&data_dir, &code, &resolved)?;
        Ok(resolved.record(&link, subject_id, poster))
    })
    .await?;
    upsert_echo_embed(pool, &record).await?;
    echo_embed(pool, link_id)
        .await?
        .ok_or_else(|| "the embed's row was not written".to_string())
}

#[derive(Clone)]
struct Download {
    app: AppHandle,
    bearer: EmbedBearer,
    data_dir: PathBuf,
    subject_id: i64,
    code: String,
    link_id: String,
    retrieve: String,
}

impl Download {
    async fn run(
        self,
        flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<EmbedRow, String> {
        let pool = open_pool().await?;
        let row = cached_or_resolved(
            &pool,
            &self.bearer,
            &self.data_dir,
            self.subject_id,
            &self.code,
            &self.link_id,
            &self.retrieve,
        )
        .await?;
        let shown = current(&pool, &self.data_dir, row.clone()).await?;
        if shown.file_rel.is_some() {
            return Ok(shown);
        }
        if row.restrict_download {
            return Err(RESTRICTED.to_string());
        }
        // The same recording embedded on another page is downloaded once.
        for rel in echo_embed_file_of_media(&pool, self.subject_id, &row.media_id).await? {
            if self.data_dir.join(&rel).is_file() {
                set_echo_embed_file(&pool, &self.link_id, Some(&rel)).await?;
                return self.reread(&pool).await;
            }
        }

        let known = (row.media_id.clone(), row.secure_link_id.clone());
        let resource_dir = self.app.path().resource_dir().ok();
        let job = self.clone();
        let flag = std::sync::Arc::clone(flag);
        let (resolved, saved) = crate::runtime::blocking::run(move || {
            let ffmpeg = echo360::find_ffmpeg(resource_dir)
                .ok_or("ffmpeg not found — run `bun run ffmpeg` in app/")?;
            let resolved = properties_for(&job.data_dir, &job.retrieve, Some(known), &job.bearer)?;
            if resolved.info.restrict_download {
                return Ok((resolved, None));
            }
            let saved = job.fetch(&resolved, &ffmpeg, &|| flag.load(Ordering::Relaxed))?;
            Ok((resolved, Some(saved)))
        })
        .await?;

        // Fresh properties may rename the video or change its policy.
        upsert_echo_embed(
            &pool,
            &resolved.record(&self.link_id, self.subject_id, None),
        )
        .await?;
        let Some(rel) = saved else {
            return Err(RESTRICTED.to_string());
        };
        set_echo_embed_file(&pool, &self.link_id, Some(&rel)).await?;
        self.reread(&pool).await
    }

    /// Find the signed files, stream and join them into the library, and
    /// report the new file as a `scrape-file` so the frontend writes its
    /// `files` row. Returns the data-dir-relative path.
    fn fetch(
        &self,
        resolved: &Resolved,
        ffmpeg: &Path,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<String, String> {
        let info = &resolved.info;
        let files = embed::media_files(info)?;
        let captions = info
            .captions_url
            .as_deref()
            .map(|url| embed::sign(url, &info.query_strings))
            .transpose()?;
        let rel = video_rel(&self.code, &info.name, &resolved.media_id, &|rel| {
            self.data_dir.join(rel).exists()
        })?;
        let dest = self.data_dir.join(&rel);
        let existed = dest.is_file();
        let (app, link_id) = (&self.app, self.link_id.as_str());
        let size = embed::download(
            &embed::Job {
                files: &files,
                captions_url: captions.as_deref(),
                ffmpeg,
                scratch: &self.data_dir.join("tmp").join("echo-embeds").join(link_id),
                dest: &dest,
            },
            &|pct| emit(app, link_id, pct, "downloading"),
            &|| emit(app, link_id, 100, "muxing"),
            cancelled,
        )?;
        let name = rel.rsplit('/').next().unwrap_or(&rel);
        app.emit(
            "scrape-file",
            &FileEvent {
                subject_id: self.subject_id,
                code: self.code.clone(),
                relative_path: rel.clone(),
                size_bytes: size,
                category: paths::category_from_path(&format!("files/{name}")).to_string(),
                canvas_id: None,
                source_url: Some(self.retrieve.clone()),
                action: if existed { "updated" } else { "new" },
            },
        )
        .ok();
        Ok(rel)
    }

    async fn reread(&self, pool: &SqlitePool) -> Result<EmbedRow, String> {
        let row = echo_embed(pool, &self.link_id)
            .await?
            .ok_or_else(|| "the embed's row is gone".to_string())?;
        current(pool, &self.data_dir, row).await
    }
}

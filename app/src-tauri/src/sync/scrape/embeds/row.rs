//! The row the embed card is drawn from.

use std::path::Path;

use sqlx::SqlitePool;

use crate::db::store::{set_echo_embed_file, EchoEmbed};

/// What `echo360_embed_info` and `echo360_embed_download` return. Paths are
/// data-dir-relative and present only while the file is on disk.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedRow {
    pub link_id: String,
    pub media_id: String,
    pub name: String,
    pub duration_s: Option<f64>,
    pub restrict_download: bool,
    pub has_captions: bool,
    pub poster_rel: Option<String>,
    pub file_rel: Option<String>,
    /// Video sources beyond the one downloaded (a room camera, say).
    pub extra_sources: i64,
}

/// The row as it stands on disk: a downloaded file the student has since
/// deleted is forgotten, so the card offers Download again.
pub(super) async fn current(
    pool: &SqlitePool,
    data_dir: &Path,
    embed: EchoEmbed,
) -> Result<EmbedRow, String> {
    let on_disk =
        |rel: &Option<String>| rel.as_ref().filter(|r| data_dir.join(r).is_file()).cloned();
    let file_rel = on_disk(&embed.file_rel);
    if embed.file_rel.is_some() && file_rel.is_none() {
        set_echo_embed_file(pool, &embed.link_id, None).await?;
    }
    Ok(EmbedRow {
        poster_rel: on_disk(&embed.poster_rel),
        file_rel,
        link_id: embed.link_id,
        media_id: embed.media_id,
        name: embed.name,
        duration_s: embed.duration_s,
        restrict_download: embed.restrict_download,
        has_captions: embed.has_captions,
        extra_sources: embed.extra_sources,
    })
}

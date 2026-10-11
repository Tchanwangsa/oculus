//! Module videos: listed by a sync, downloaded on request.

use crate::library::paths;
use crate::sync::office::is_video;
use crate::sync::render::{content_type_of, file_name, locked_until, modified_of};
use crate::sync::{Engine, FileEvent, Subject};

impl Engine {
    /// Download one module video on request, past the size cap a sync
    /// applies. Progress is whole percents; `cancelled` is polled per chunk
    /// and ends the download with [`crate::sources::canvas::CANCELLED`].
    pub fn download_video(
        &self,
        c: &Subject,
        file_id: i64,
        on_progress: &dyn Fn(u8),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<String, String> {
        let r = self.canvas.get(&format!("/api/v1/files/{file_id}"))?;
        if !r.ok() {
            return Err(format!(
                "Canvas answered HTTP {} for file {file_id}",
                r.status
            ));
        }
        let info = r.json()?;
        let name = file_name(&info, None);
        if !is_video(&content_type_of(&info), &name) {
            return Err(format!("{name} is not a video"));
        }
        if let Some(until) = locked_until(&info) {
            return Err(format!("{name} is locked{until}"));
        }
        self.save_video(c, file_id, &info, &name, on_progress, cancelled)
    }

    /// Stream a video into `files/` through a `.part` sibling, renamed on
    /// success and removed on any failure, so a cut-off download never looks
    /// finished. Never buffered: lecture recordings run to hundreds of MB.
    fn save_video(
        &self,
        c: &Subject,
        file_id: i64,
        info: &serde_json::Value,
        name: &str,
        on_progress: &dyn Fn(u8),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<String, String> {
        let rel = paths::course_rel_path(&c.code, &format!("files/{name}"))
            .ok_or_else(|| format!("invalid file name: {name}"))?;
        let dest = self.data_dir.join(&rel);
        let part = dest.with_file_name(format!(
            "{}.part",
            dest.file_name().and_then(|n| n.to_str()).unwrap_or("video")
        ));
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let url = self
            .download_url(info, file_id)?
            .ok_or_else(|| "Canvas gave no download URL".to_string())?;

        let existed = dest.is_file();
        let size = self
            .canvas
            .download_to(&url, &part, on_progress, cancelled)
            .and_then(|n| {
                std::fs::rename(&part, &dest)
                    .map(|_| n)
                    .map_err(|e| e.to_string())
            })
            .inspect_err(|_| {
                let _ = std::fs::remove_file(&part);
            })?;

        self.reporter.file(&FileEvent {
            subject_id: c.id,
            code: c.code.clone(),
            relative_path: rel.clone(),
            size_bytes: size,
            category: paths::category_from_path(&format!("files/{name}")).to_string(),
            canvas_id: Some(file_id),
            source_url: None,
            action: if existed { "updated" } else { "new" },
        });
        let modified = modified_of(info);
        if !modified.is_empty() {
            self.record_manifest(file_id, (modified, info["size"].as_u64().unwrap_or(size)));
        }
        Ok(rel)
    }
}

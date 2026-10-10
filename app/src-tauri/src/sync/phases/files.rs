//! Fetching one Canvas file: downloads, the manifest, and the skip rules.

use std::collections::HashMap;

use crate::library::paths;
use crate::sync::office::{
    is_csv_type, is_generic_binary, is_sheet_type, is_video, office_ext, office_ext_of,
    office_to_pdf,
};
use crate::sync::render::{content_type_of, file_name, locked_until, modified_of};
use crate::sync::{
    Engine, Fetched, FileEvent, FileFailed, FileStart, Subject, DOWNLOADABLE_TYPES, MAX_FILE_BYTES,
};

impl Engine {
    /// Download one Canvas file if its type is allowlisted. A video is only
    /// recorded — a sync never downloads one ([`Engine::download_video`] does).
    pub(in crate::sync) fn fetch_file(
        &self,
        c: &Subject,
        file_id: i64,
        display: Option<&str>,
    ) -> Result<Fetched, String> {
        let r = self.canvas.get(&format!("/api/v1/files/{file_id}"))?;
        if !r.ok() {
            return Ok(Fetched::Skipped);
        }
        let info = r.json()?;
        let name = file_name(&info, display);

        // Canvas serves some uploads as a generic binary (whatever the
        // uploader's browser claimed); those fall back to the extension.
        let ct = content_type_of(&info);
        let video = is_video(&ct, &name);
        let office = office_ext(&ct).or_else(|| {
            is_generic_binary(&ct)
                .then(|| office_ext_of(&name))
                .flatten()
        });
        let sheet = is_sheet_type(&ct, &name) || is_csv_type(&ct, &name);
        let downloadable = DOWNLOADABLE_TYPES.contains(&ct.as_str())
            || (is_generic_binary(&ct) && paths::is_pdf(&name));
        if !video && !downloadable && office.is_none() && !sheet {
            return Ok(Fetched::Skipped);
        }
        if !video && info["size"].as_u64().unwrap_or(0) > MAX_FILE_BYTES {
            self.reporter.log(
                "warning",
                &c.code,
                &format!("file {file_id}: over size cap, skipped"),
            );
            return Ok(Fetched::Skipped);
        }
        if let Some(until) = locked_until(&info) {
            let name = info["display_name"].as_str().or(display).unwrap_or("file");
            self.reporter
                .log("info", &c.code, &format!("{name}: locked{until}, skipped"));
            return Ok(Fetched::Skipped);
        }
        let Some(rel) = paths::course_rel_path(&c.code, &format!("files/{name}")) else {
            return Ok(Fetched::Skipped);
        };

        // Unchanged since last time and on disk (derived PDF too) → skip.
        let modified = modified_of(&info);
        let meta_size = info["size"].as_u64().unwrap_or(0);
        let known = !modified.is_empty()
            && self
                .manifest
                .borrow()
                .get(&file_id.to_string())
                .is_some_and(|(m, s)| *m == modified && *s == meta_size);
        let on_disk = self.data_dir.join(&rel).is_file()
            && paths::doc_pdf_rel(&rel).map_or(true, |p| self.data_dir.join(p).is_file());
        let unchanged = || {
            self.reporter.file(&FileEvent {
                subject_id: c.id,
                code: c.code.clone(),
                relative_path: rel.clone(),
                size_bytes: meta_size,
                category: paths::category_from_path(&format!("files/{name}")).to_string(),
                canvas_id: Some(file_id),
                source_url: None,
                action: "unchanged",
            });
        };

        if video {
            // A copy Canvas has since changed is left as it is: the student
            // chose to download it, and a sync never does.
            if known && on_disk {
                unchanged();
            }
            return Ok(Fetched::Video {
                rel,
                canvas_id: file_id,
            });
        }

        if known && on_disk {
            unchanged();
            // Unchanged bytes with no text of their own, or PDF-route files
            // beside them, are converted again without a download.
            if sheet && crate::pages::sheets::needs_conversion(&self.data_dir, &rel) {
                self.convert_sheet(&c.code, &rel, c.id);
            }
            return Ok(Fetched::Saved(rel));
        }

        // Resolved before the announcement: no URL means nothing to download,
        // and a file announced as downloading must end in an event.
        let Some(url) = self.download_url(&info, file_id)? else {
            return Ok(Fetched::Skipped);
        };

        self.reporter.file_start(&FileStart {
            subject_id: c.id,
            code: c.code.clone(),
            relative_path: rel.clone(),
            filename: name.clone(),
            size_bytes: meta_size,
        });
        let failed = |error: String| {
            self.reporter.file_failed(&FileFailed {
                subject_id: c.id,
                relative_path: rel.clone(),
                error: error.clone(),
            });
            error
        };

        let bytes = self
            .fetch_bytes(&url)
            .map_err(|e| failed(format!("Download failed: {e}.")))?;

        // The original is the library file. Office documents get a derived
        // "deck.pptx.pdf" beside them — never announced, never a database row
        // — and spreadsheets their text, "marks.xlsx.md".
        let (rel, action) = self
            .store(c, &format!("files/{name}"), &bytes, Some(file_id), None)
            .map_err(|e| failed(format!("Could not save the file: {e}.")))?;

        // A failed Office conversion stays out of the manifest so it retries.
        let mut complete = true;
        if let Some(ext) = office {
            match office_to_pdf(&bytes, ext) {
                Ok(pdf) => {
                    paths::write_course_bytes(
                        &self.data_dir,
                        &c.code,
                        &format!("files/{name}.pdf"),
                        &pdf,
                    )?;
                    if self.parse_pdfs {
                        self.trigger_parse(&rel, c.id);
                    }
                }
                Err(e) => {
                    complete = false;
                    self.reporter.log(
                        "warning",
                        &c.code,
                        &format!("{name}: PDF conversion failed — stored original only ({e})"),
                    );
                    self.conversion_failed(&rel, action, c.id);
                }
            }
        }
        if sheet {
            self.convert_sheet(&c.code, &rel, c.id);
        }
        if complete && !modified.is_empty() {
            self.manifest
                .borrow_mut()
                .insert(file_id.to_string(), (modified, meta_size));
        }
        Ok(Fetched::Saved(rel))
    }

    /// Add one entry to `file-manifest.json` as it is on disk now: a download
    /// runs for minutes, and a sync may have saved the manifest meanwhile.
    pub(in crate::sync) fn record_manifest(&self, file_id: i64, entry: (String, u64)) {
        let path = self.data_dir.join("file-manifest.json");
        let mut on_disk: HashMap<String, (String, u64)> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        on_disk.insert(file_id.to_string(), entry.clone());
        self.manifest
            .borrow_mut()
            .insert(file_id.to_string(), entry);
        if let Ok(json) = serde_json::to_string(&on_disk) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Canvas file URLs redirect to a CDN that rejects our cookie, so prefer
    /// the signed `public_url`. An empty `info.url` must be refused: it would
    /// resolve to the Canvas home page and be saved as the file.
    pub(in crate::sync) fn download_url(
        &self,
        info: &serde_json::Value,
        file_id: i64,
    ) -> Result<Option<String>, String> {
        if let Ok(r) = self
            .canvas
            .get(&format!("/api/v1/files/{file_id}/public_url"))
        {
            if r.ok() {
                if let Ok(j) = r.json() {
                    if let Some(u) = j["public_url"].as_str().filter(|u| !u.is_empty()) {
                        return Ok(Some(u.to_string()));
                    }
                }
            }
        }
        Ok(info["url"]
            .as_str()
            .filter(|u| !u.is_empty())
            .map(str::to_string))
    }

    /// Errors never carry the URL: a download URL is signed, and these
    /// sentences reach the log and the UI.
    pub(in crate::sync) fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let r = self
            .canvas
            .get(url)
            .map_err(|_| "could not reach the file server".to_string())?;
        if !r.ok() {
            return Err(format!("the file server answered HTTP {}", r.status));
        }
        // A login page where a file should be means the session lapsed
        // mid-run; saving it would quietly corrupt the library.
        if r.content_type.contains("text/html") {
            return Err("got HTML instead of the file — session or URL problem".to_string());
        }
        Ok(r.body)
    }
}

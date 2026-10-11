//! One chunk of tasks: submit, upload, poll, and collect each result.

use super::archive::safe_extract;
use super::document::CloudDocument;
use super::errors::is_batch_id;
use super::task::{basename, scope_of, Scope, Task};
use super::MinerUCloud;
use super::{FIRST_POLL_DELAY, MAX_POLL_DELAY, POLL_DEADLINE};
use crate::parse::{ParseError, Phase};
use crate::providers::ratelimit::nap;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

pub(super) fn fail_document(
    failures: &mut [Option<ParseError>],
    remaining: &mut HashMap<&str, &Task>,
    task: &Task,
    error: ParseError,
) {
    failures[task.document] = Some(error);
    // Its other ranges are pointless now; stop polling for them.
    remaining.retain(|_, queued| queued.document != task.document);
}

impl MinerUCloud {
    /// Submit, upload and poll one chunk of up to `MAX_FILES_PER_BATCH` tasks.
    ///
    /// `Err` is a batch-level failure — every document that still has work
    /// outstanding fails with it. A document-level failure is written into
    /// `failures` and the rest of the chunk carries on.
    pub(super) fn run_batch(
        &self,
        tasks: &[Task],
        documents: &[Arc<CloudDocument>],
        failures: &mut [Option<ParseError>],
        completed: &mut HashSet<String>,
        workspace: &Path,
    ) -> Result<(), ParseError> {
        fs::create_dir_all(workspace)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", workspace.display())))?;

        // A skipped document is dropped before anything is reserved or sent.
        for task in tasks {
            if failures[task.document].is_none() && documents[task.document].cancelled() {
                failures[task.document] = Some(ParseError::Cancelled);
            }
        }
        // Documents that already failed earlier in this batch are not sent.
        let live: Vec<&Task> = tasks
            .iter()
            .filter(|task| failures[task.document].is_none())
            .collect();
        if live.is_empty() {
            return Ok(());
        }

        let body = json!({
            "files": live.iter().map(|task| task.api_entry()).collect::<Vec<_>>(),
            // Fixed: the library's records were built with these, so changing
            // one is a re-parse, not a setting. `"ch"` is MinerU's multilingual
            // default.
            "model_version": "pipeline",
            "enable_formula": true,
            "enable_table": true,
            "language": "ch",
        });

        // Reserved before the call and never given back: the server may have
        // counted the work. See `ledger`.
        self.ledger.record(
            live.len() as u64,
            live.iter().map(|t| u64::from(t.page_count)).sum(),
        )?;

        let data = self.api_json("POST", "/file-urls/batch", Some(body), &self.submit)?;
        let urls: Vec<&str> = data
            .get("file_urls")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if urls.len() != live.len() {
            return Err(ParseError::Document {
                code: "upload-url-count".into(),
            });
        }
        let batch_id = data
            .get("batch_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        // It goes into the poll's path, and keyd refuses a path with anything
        // else in it. Only this batch's documents fail; the next batch runs.
        if !is_batch_id(batch_id) {
            for task in &live {
                failures[task.document].get_or_insert(ParseError::Document {
                    code: "batch-id".into(),
                });
            }
            return Ok(());
        }

        // Smallest file first: extraction starts only once every PUT is in,
        // so the small ones must not queue behind a slow large upload.
        let mut uploads: Vec<(&Task, &str)> = live.iter().copied().zip(urls).collect();
        uploads.sort_by_key(|(task, _)| documents[task.document].bytes());
        // A document over several tasks uploads its whole file once per task.
        let mut puts_left: HashMap<usize, u32> = HashMap::new();
        for (task, _) in &uploads {
            *puts_left.entry(task.document).or_default() += 1;
        }
        for (&index, &puts) in &puts_left {
            documents[index].await_upload(documents[index].bytes() * u64::from(puts));
        }

        let mut remaining: HashMap<&str, &Task> = HashMap::new();
        for (task, url) in uploads {
            if failures[task.document].is_some() {
                continue;
            }
            let document = &documents[task.document];
            if document.cancelled() {
                fail_document(failures, &mut remaining, task, ParseError::Cancelled);
                continue;
            }
            document.set_phase(Phase::Uploading);
            let before = document.bytes_done();
            let uploaded = self.put_file(
                url,
                &task.source,
                &|sent| document.set_bytes_done(before + sent),
                &|| document.cancelled(),
            );
            match uploaded {
                Ok(()) => {
                    remaining.insert(task.data_id.as_str(), task);
                    let left = puts_left.entry(task.document).or_default();
                    *left = left.saturating_sub(1);
                    if *left == 0 {
                        document.set_phase(Phase::Processing);
                    }
                }
                Err(error) if scope_of(&error) == Scope::Batch => return Err(error),
                Err(error) => fail_document(failures, &mut remaining, task, error),
            }
        }

        let by_name: HashMap<&str, &Task> = live
            .iter()
            .map(|task| (task.upload_name.as_str(), *task))
            .collect();
        let deadline = Instant::now() + POLL_DEADLINE.mul_f64(self.time_scale);
        let mut delay = FIRST_POLL_DELAY;
        loop {
            // A document skipped mid-poll is dropped; its result is never fetched.
            let skipped: Vec<&Task> = remaining
                .values()
                .copied()
                .filter(|task| documents[task.document].cancelled())
                .collect();
            for task in skipped {
                fail_document(failures, &mut remaining, task, ParseError::Cancelled);
            }
            if remaining.is_empty() {
                break;
            }
            if Instant::now() >= deadline {
                // Not `Document`: a stall is worth retrying later.
                return Err(ParseError::Offline(
                    "batch timed out after 60 minutes".into(),
                ));
            }
            let data = self.api_json(
                "GET",
                &format!("/extract-results/batch/{batch_id}"),
                None,
                &self.poll,
            )?;

            for result in data
                .get("extract_result")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
            {
                let by_id = result.get("data_id").and_then(Value::as_str);
                let task = by_id.and_then(|id| remaining.get(id).copied()).or_else(|| {
                    result
                        .get("file_name")
                        .and_then(Value::as_str)
                        .and_then(|name| by_name.get(name).copied())
                });
                let Some(task) = task else { continue };
                if !remaining.contains_key(task.data_id.as_str()) {
                    continue;
                }
                let document = &documents[task.document];
                if document.cancelled() {
                    fail_document(failures, &mut remaining, task, ParseError::Cancelled);
                    continue;
                }
                let state = result
                    .get("state")
                    .and_then(Value::as_str)
                    .unwrap_or_default();

                if state == "running" {
                    let extracted = result
                        .get("extract_progress")
                        .and_then(|value| value.get("extracted_pages"))
                        .and_then(Value::as_u64)
                        .unwrap_or_default() as u32;
                    document.report(&task.data_id, extracted, task.page_count);
                    continue;
                }
                if state == "failed" {
                    fail_document(
                        failures,
                        &mut remaining,
                        task,
                        ParseError::Document {
                            code: "task-failed".into(),
                        },
                    );
                    continue;
                }
                if state != "done" {
                    continue;
                }

                let zip_url = result
                    .get("full_zip_url")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if zip_url.is_empty() {
                    fail_document(
                        failures,
                        &mut remaining,
                        task,
                        ParseError::Document {
                            code: "missing-result".into(),
                        },
                    );
                    continue;
                }
                match self.collect_result(task, document, zip_url, workspace) {
                    Ok(()) => {
                        completed.insert(task.data_id.clone());
                        remaining.remove(task.data_id.as_str());
                        // Full length, even if `extract_progress` never came.
                        document.report(&task.data_id, task.page_count, task.page_count);
                    }
                    Err(error) if scope_of(&error) == Scope::Batch => return Err(error),
                    Err(error) => fail_document(failures, &mut remaining, task, error),
                }
            }

            if !remaining.is_empty() {
                nap(delay, self.time_scale);
                delay = delay.mul_f64(1.4).min(MAX_POLL_DELAY);
            }
        }
        Ok(())
    }

    /// Download one task's result, rebase its page indices onto the document,
    /// and stage the crops it references.
    fn collect_result(
        &self,
        task: &Task,
        document: &CloudDocument,
        zip_url: &str,
        workspace: &Path,
    ) -> Result<(), ParseError> {
        let result_dir = workspace.join(&task.data_id);
        let zip_path = workspace.join(format!("{}.zip", task.data_id));
        self.download_zip(zip_url, &zip_path)?;
        safe_extract(&zip_path, &result_dir)?;

        let (content_path, items) = crate::parse::mineru::content_list(&result_dir)?;

        let source_images = content_path.parent().unwrap_or(&result_dir).join("images");
        let staging = document.source_images();
        fs::create_dir_all(&staging)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", staging.display())))?;

        let mut renamed: HashMap<String, String> = HashMap::new();
        let mut rebased = Vec::with_capacity(items.len());
        for item in &items {
            let Some(object) = item.as_object() else {
                return Err(ParseError::Document {
                    code: "invalid-content-list".into(),
                });
            };
            let mut absolute = object.clone();

            let page_idx = match object.get("page_idx") {
                None | Some(Value::Null) => 0,
                Some(value) => value
                    .as_i64()
                    .or_else(|| value.as_f64().map(|f| f as i64))
                    .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
                    .ok_or(ParseError::Document {
                        code: "invalid-page-index".into(),
                    })?,
            };
            // Hard error: `ParseOutput::new` would gap-fill a wrong offset
            // into a quietly half-empty document.
            if page_idx < 0 || page_idx >= i64::from(task.page_count) {
                return Err(ParseError::Document {
                    code: "page-index-out-of-range".into(),
                });
            }
            absolute.insert(
                "page_idx".into(),
                Value::from(page_idx + i64::from(task.page_offset)),
            );

            if let Some(image) = object
                .get("img_path")
                .and_then(Value::as_str)
                .filter(|p| !p.is_empty())
            {
                let original = basename(image);
                if let Some(target) = renamed.get(&original) {
                    // The same crop cited twice in one task keeps one copy.
                    absolute.insert("img_path".into(), Value::from(format!("images/{target}")));
                    rebased.push(Value::Object(absolute));
                    continue;
                }
                let mut target = original.clone();
                let mut destination = staging.join(&target);
                if destination.exists() {
                    // Two tasks both named `4.jpg`; the page offset differs.
                    target = format!("p{}_{original}", task.page_offset + 1);
                    destination = staging.join(&target);
                }
                let source = source_images.join(&original);
                // A crop the result named but did not ship leaves `img_path`
                // as it was; `render` skips links with no file behind them.
                if source.is_file() {
                    fs::copy(&source, &destination).map_err(|e| {
                        ParseError::Io(format!("stage {}: {e}", destination.display()))
                    })?;
                    absolute.insert("img_path".into(), Value::from(format!("images/{target}")));
                    renamed.insert(original, target);
                }
            }
            rebased.push(Value::Object(absolute));
        }

        document.push_content(rebased);
        Ok(())
    }
}

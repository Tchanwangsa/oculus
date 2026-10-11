//! Splitting documents into tasks and parsing a whole batch end to end.

use super::document::{CloudDocument, DocumentOutput};
use super::task::{data_id, Task};
use super::MinerUCloud;
use crate::parse::mineru::ledger::{MAX_FILES_PER_BATCH, MAX_FILE_BYTES};
use crate::parse::mineru::{render, WorkDir};
use crate::parse::{check_size, ParseError};
use std::collections::HashSet;
use std::sync::Arc;

impl MinerUCloud {
    pub(super) fn build_tasks(
        &self,
        index: usize,
        document: &CloudDocument,
    ) -> Result<Vec<Task>, ParseError> {
        let path = document.path().to_path_buf();
        check_size(&path, MAX_FILE_BYTES)?;

        let total = document.count_pages()?;
        document.set_total_pages(total);

        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let suffix = path
            .extension()
            .map(|s| format!(".{}", s.to_string_lossy()))
            .unwrap_or_default();

        let mut tasks = Vec::new();
        let mut start = 0;
        while start < total {
            let end = (start + self.pages_per_task).min(total);
            tasks.push(Task {
                document: index,
                data_id: data_id(),
                // Unique per task, because `file_name` is the fallback key
                // when MinerU echoes a result without its `data_id`.
                upload_name: format!("{stem}__oculus_{}_{end}{suffix}", start + 1),
                source: path.clone(),
                page_offset: start,
                page_count: end - start,
                // Absent for a document that fits in one task: the server
                // treats "no range" as the whole file.
                page_ranges: (total > self.pages_per_task).then(|| format!("{}-{end}", start + 1)),
            });
            start = end;
        }
        Ok(tasks)
    }

    /// Parse every document in one batch, returning one result each, in order.
    pub fn extract_documents(
        &self,
        documents: &[Arc<CloudDocument>],
    ) -> Vec<Result<DocumentOutput, ParseError>> {
        // For the zips and the staged crops.
        let workspace = match WorkDir::new(format!("mineru-cloud-{}", data_id())) {
            Ok(workspace) => workspace,
            Err(error) => return documents.iter().map(|_| Err(error.clone())).collect(),
        };

        let mut failures: Vec<Option<ParseError>> = documents.iter().map(|_| None).collect();
        let mut tasks: Vec<Task> = Vec::new();
        for (index, document) in documents.iter().enumerate() {
            if document.cancelled() {
                failures[index] = Some(ParseError::Cancelled);
                continue;
            }
            document.set_source_images(workspace.path().join(data_id()).join("images"));
            match self.build_tasks(index, document) {
                Ok(built) => tasks.extend(built),
                // Unreadable, empty or oversized: this file only.
                Err(error) => failures[index] = Some(error),
            }
        }

        let mut batch_error = None;
        let mut completed: HashSet<String> = HashSet::new();
        // The whole batch's reservation is checked before any of it is sent,
        // so a batch that cannot fit today does not half-send.
        if let Err(error) = self.ledger.ensure_available(tasks.len() as u64) {
            batch_error = Some(error);
        } else {
            for (chunk, slice) in tasks.chunks(MAX_FILES_PER_BATCH).enumerate() {
                let folder = workspace.path().join(format!("batch-{chunk}"));
                if let Err(error) =
                    self.run_batch(slice, documents, &mut failures, &mut completed, &folder)
                {
                    batch_error = Some(error);
                    break;
                }
            }
        }

        documents
            .iter()
            .enumerate()
            .map(|(index, document)| {
                // Before `render`, which would write into a staging directory
                // its abandoned caller has already removed.
                if document.cancelled() {
                    return Err(ParseError::Cancelled);
                }
                if let Some(error) = failures[index].take() {
                    return Err(error);
                }
                let outstanding = tasks
                    .iter()
                    .any(|task| task.document == index && !completed.contains(&task.data_id));
                if outstanding {
                    return Err(batch_error.clone().unwrap_or(ParseError::Document {
                        code: "incomplete".into(),
                    }));
                }
                let total = document.total_pages();
                render::render(
                    &document.take_content(),
                    total,
                    &document.source_images(),
                    &document.images_dir,
                    &document.images_rel,
                )
                .map(|(pages, image_count)| DocumentOutput {
                    pages,
                    image_count,
                    total_pages: total,
                })
            })
            .collect()
    }
}

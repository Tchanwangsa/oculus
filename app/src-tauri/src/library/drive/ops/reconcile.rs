//! Rows for the student's own files that arrived without one: written by a
//! chat agent, Finder or another app straight into a subject's folder. Such a
//! file lists in Drive (the listing reads the disk) but, with no `files` row,
//! is never parsed or searched. `reconcile_own_files` inserts a row for each
//! own file that has none; it never updates or deletes a row. The frontend
//! kicks the parse of what it returns (`app/src/lib/drive/reconcile.ts`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sqlx::SqlitePool;
use tauri::AppHandle;

use super::error::{DriveError, DriveResult};
use super::events::{emit, DriveChange};
use super::rows::{file_type, name_of, subject_id};
use crate::library::files::uploads::ImportedFile;
use crate::library::layout::{self, DerivedKind};
use crate::library::paths::{artifact_at, doc_pdf_rel, parse_artifacts, safe_dir, ArtifactSlot};
use crate::parse::InFlight;

/// Every own file of the subject `code` that `known` (the subject's row
/// paths) lacks, as the row it should get. Left out, as Drive leaves them
/// out: another file's parse artifacts and converted PDF, and the markdown a
/// parse running right now is writing before its record says whose it is.
pub fn unrowed_own_files(
    data_dir: &Path,
    code: &str,
    known: &HashSet<String>,
) -> Vec<ImportedFile> {
    let files = layout::own_files(data_dir, code);
    let derived = derived_paths(data_dir, files.iter().map(|(rel, _)| rel.as_str()));
    files
        .into_iter()
        .filter(|(rel, _)| !known.contains(rel) && layout::own_path_ok(rel))
        .filter(|(rel, _)| {
            let path = data_dir.join(rel);
            !derived.iter().any(|d| path.starts_with(d))
        })
        .map(|(rel, meta)| {
            let kind = file_type(name_of(&rel));
            ImportedFile::new(rel, &kind, meta.len())
        })
        .collect()
}

/// What `files` carry with them, on disk: parse artifacts (a folder's files
/// with it), converted PDFs, and the markdown of a parse in flight.
fn derived_paths<'a>(data_dir: &Path, files: impl Iterator<Item = &'a str>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for rel in files {
        out.extend(parse_artifacts(data_dir, rel));
        out.extend(
            doc_pdf_rel(rel)
                .filter(|pdf| pdf != rel)
                .map(|pdf| data_dir.join(pdf)),
        );
        if parsing(data_dir, rel) {
            out.extend(artifact_at(
                data_dir,
                rel,
                ArtifactSlot::Derived(DerivedKind::Markdown),
            ));
        }
    }
    out
}

/// A parse holds this file's PDF right now. Asked by claiming it and letting
/// go at once, so nothing waits.
fn parsing(data_dir: &Path, rel: &str) -> bool {
    doc_pdf_rel(rel).is_some()
        && InFlight::shared()
            .try_claim_all(&[crate::sync::parse_key(data_dir, rel)])
            .is_err()
}

/// Insert the rows `unrowed_own_files` finds for `code`; returns the
/// subject's id and the rows inserted. A row another writer added meanwhile
/// wins (`INSERT OR IGNORE` on the subject and path).
pub async fn reconcile_own_files_at(
    data_dir: &Path,
    pool: &SqlitePool,
    code: &str,
) -> DriveResult<(i64, Vec<ImportedFile>)> {
    let db = |e: sqlx::Error| DriveError::io(format!("the database refused it — {e}"));
    let Some(subject) = subject_id(pool, &safe_dir(code)).await? else {
        return Err(DriveError::io(format!("no subject {code}")));
    };
    let known: HashSet<String> =
        sqlx::query_scalar::<_, String>("SELECT relative_path FROM files WHERE subject_id = ?1")
            .bind(subject)
            .fetch_all(pool)
            .await
            .map_err(db)?
            .into_iter()
            .collect();
    let (dir, owned) = (data_dir.to_path_buf(), code.to_string());
    let found = crate::runtime::blocking::run(move || {
        Ok::<_, String>(unrowed_own_files(&dir, &owned, &known))
    })
    .await
    .map_err(DriveError::io)?;

    let mut inserted = Vec::new();
    let mut tx = pool.begin().await.map_err(db)?;
    for file in found {
        let done = sqlx::query(
            "INSERT OR IGNORE INTO files (subject_id, filename, relative_path, file_type,
                size_bytes, category, first_seen_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), datetime('now'))",
        )
        .bind(subject)
        .bind(&file.filename)
        .bind(&file.relative_path)
        .bind(&file.file_type)
        .bind(file.size_bytes as i64)
        .bind(&file.category)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        if done.rows_affected() > 0 {
            inserted.push(file);
        }
    }
    tx.commit().await.map_err(db)?;
    Ok((subject, inserted))
}

/// Rows for the subject's own files that have none, announced on
/// `drive-changed` (`added`) when there were any; a run that inserts nothing
/// announces nothing, so a listing reloading on the event settles.
#[tauri::command]
pub async fn reconcile_own_files(
    app: AppHandle,
    subject_code: String,
) -> Result<Vec<ImportedFile>, String> {
    let pool = crate::db::store::open_pool()
        .await
        .map_err(DriveError::io)?;
    let data_dir = crate::library::paths::data_dir();
    let result = reconcile_own_files_at(&data_dir, &pool, &subject_code).await;
    pool.close().await;
    let (subject, inserted) = result.inspect_err(|e| {
        elog!("[oculus] reconcile_own_files {subject_code}: {}", e.message);
    })?;
    if !inserted.is_empty() {
        emit(
            &app,
            &DriveChange {
                subject_ids: vec![subject],
                added: inserted.iter().map(|f| f.relative_path.clone()).collect(),
                ..DriveChange::default()
            },
        );
    }
    Ok(inserted)
}

/// The student's own files, which the scraper never writes to; downstream they
/// are ordinary library files.
pub const UPLOADS_DIR: &str = "uploads";

/// True for a data-dir-relative path inside some subject's uploads folder.
///
/// The delete command's entire guard: nothing else under `courses/` is the
/// user's to throw away.
pub fn is_upload_rel(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    !rel.contains("..") && parts.len() > 3 && parts[0] == "courses" && parts[2] == UPLOADS_DIR
}

/// The student's own notes, written in the app as markdown; the other folder
/// the scraper never writes to. Downstream, ordinary library files.
pub const DOCUMENTS_DIR: &str = "documents";

/// Where a note's pictures go: `documents/assets/`, one folder for the
///
/// The folder on disk and the `![](assets/…)` prefix a note carries. One folder
/// because the names are stamps and a per-note folder would move on rename.
pub const DOCUMENT_ASSETS_DIR: &str = "assets";

/// True only for `courses/<code>/documents/<name>.md`.
///
/// The entire guard for writing, renaming and deleting a document: exactly
/// four segments, no traversal, a `.md` at the end — which also keeps
/// `documents/assets/` out.
pub fn is_document_rel(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    !rel.contains("..")
        && parts.len() == 4
        && parts[0] == "courses"
        && !parts[1].is_empty()
        && parts[2] == DOCUMENTS_DIR
        && parts[3].len() > 3
        && parts[3].ends_with(".md")
}

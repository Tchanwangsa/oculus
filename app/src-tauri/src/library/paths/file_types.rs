/// Extensions LibreOffice converts to PDF at download, as `{name}.pdf` beside it.
/// Mirrored by `OFFICE_EXTS` in `app/src/lib/files/fileTypes.ts`.
pub const OFFICE_EXTS: &[&str] = &[".pptx", ".docx", ".ppt", ".doc"];

/// Spreadsheets and CSVs, converted to `{name}.md` text in-process
/// (`crate::pages::sheets`): never PDF-backed, never embedded. Mirrored by
/// `SHEET_EXTS` in `app/src/lib/files/fileTypes.ts`.
pub const SHEET_EXTS: &[&str] = &[".xlsx", ".xlsm", ".xls", ".ods", ".csv"];

/// A spreadsheet by name, in any case.
pub fn is_sheet(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    SHEET_EXTS.iter().any(|e| lower.ends_with(e))
}

/// A PDF by name, in any case: Canvas keeps whatever the uploader typed.
pub fn is_pdf(rel: &str) -> bool {
    rel.to_ascii_lowercase().ends_with(".pdf")
}

/// `files.file_type` values that go through parse and embed, as a SQL list for
/// `lower(file_type) IN …`; built from `OFFICE_EXTS` so no query drifts.
pub fn pdf_backed_sql_list() -> String {
    let quoted: Vec<String> = std::iter::once("pdf")
        .chain(OFFICE_EXTS.iter().map(|e| e.trim_start_matches('.')))
        .map(|e| format!("'{e}'"))
        .collect();
    format!("({})", quoted.join(", "))
}

/// The PDF that parsing, embedding and viewing use: the file itself, the
/// converted sibling for Office documents, `None` otherwise.
pub fn doc_pdf_rel(rel: &str) -> Option<String> {
    if is_pdf(rel) {
        return Some(rel.to_string());
    }
    let lower = rel.to_ascii_lowercase();
    OFFICE_EXTS
        .iter()
        .any(|e| lower.ends_with(e))
        .then(|| format!("{rel}.pdf"))
}

//! Canvas titles become filenames, so every component is sanitised.

use super::{doc_pdf_rel, is_sheet};

pub fn safe_dir(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn safe_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .replace("..", "_")
}

pub fn safe_rel_path(rel: &str) -> Option<String> {
    let parts: Vec<String> = rel
        .split('/')
        .filter(|s| !s.is_empty())
        .map(safe_filename)
        .filter(|s| s != "." && s != "_" && !s.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The data-dir-relative path an artifact will occupy, known before any bytes
/// move, so an in-flight download uses the write event's key.
pub fn course_rel_path(code: &str, rel_path: &str) -> Option<String> {
    safe_rel_path(rel_path).map(|safe| format!("courses/{}/{}", safe_dir(code), safe))
}

/// What a write did to the file on disk: the only place "nothing changed" is
/// knowable, since the scraper re-fetches everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteAction {
    New,
    Updated,
    Unchanged,
}

impl WriteAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            WriteAction::New => "new",
            WriteAction::Updated => "updated",
            WriteAction::Unchanged => "unchanged",
        }
    }
}

/// Write one course artifact; returns the data-dir-relative path, the byte count
/// and whether the content was new, changed or identical (and not rewritten).
pub fn write_course_bytes(
    data_dir: &std::path::Path,
    code: &str,
    rel_path: &str,
    content: &[u8],
) -> Result<(String, u64, WriteAction), String> {
    let rel = course_rel_path(code, rel_path).ok_or_else(|| format!("invalid path: {rel_path}"))?;
    let path = data_dir.join(&rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let action = match std::fs::read(&path) {
        Ok(existing) if existing == content => WriteAction::Unchanged,
        Ok(_) => WriteAction::Updated,
        Err(_) => WriteAction::New,
    };
    if action != WriteAction::Unchanged {
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
    }
    Ok((rel, content.len() as u64, action))
}

/// Delete a file's derived text and parse/embed artifacts (`{stem}.md`,
/// `.pages.json`, `.emb.json`, `{stem}_images/`). The skip checks read those
/// records, not the source, so changed bytes would otherwise keep the old text.
/// A spreadsheet's stem is its own name, and a `{name}.pdf` beside it is a
/// PDF-route leftover that goes too. `library_rel` is data-dir-relative.
pub fn purge_parse_artifacts(data_dir: &std::path::Path, library_rel: &str) {
    let sheet = is_sheet(library_rel);
    let Some(pdf_rel) =
        doc_pdf_rel(library_rel).or_else(|| sheet.then(|| format!("{library_rel}.pdf")))
    else {
        return;
    };
    let pdf = data_dir.join(&pdf_rel);
    let (Some(stem), Some(parent)) = (pdf.file_stem().and_then(|s| s.to_str()), pdf.parent())
    else {
        return;
    };
    for name in [
        format!("{stem}.md"),
        format!("{stem}.pages.json"),
        format!("{stem}.emb.json"),
    ] {
        let _ = std::fs::remove_file(parent.join(name));
    }
    let _ = std::fs::remove_dir_all(parent.join(format!("{stem}_images")));
    if sheet {
        let _ = std::fs::remove_file(&pdf);
    }
}

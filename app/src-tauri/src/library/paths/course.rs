//! Canvas titles become filenames, so every component is sanitised.

use super::{doc_pdf_rel, is_sheet};

/// Canvas titles become filenames, so every component of a course artifact
/// path is sanitised.
pub fn safe_dir(s: &str) -> String {
    let filename = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    safe_filename(&filename)
}

/// A subject code for comparing: upper-case, with space, `-` and `_` alike,
/// so `"cs 101"` meets the `CS_101` folder `safe_dir` made of `"CS 101"`.
pub fn fold_code(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            ' ' | '-' => '_',
            c => c.to_ascii_uppercase(),
        })
        .collect()
}

pub fn safe_filename(s: &str) -> String {
    let filename = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .replace("..", "_");
    let filename = filename.trim_end_matches(['.', ' ']);
    let stem = filename
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    let device = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        });
    if device {
        format!("_{filename}")
    } else {
        filename.to_owned()
    }
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
    safe_rel_path(rel_path).map(|safe| crate::library::layout::in_course(code, &safe))
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

/// Delete a file's derived text and parse/embed artifacts (`parse_artifacts`).
/// The skip checks read those records, not the source, so changed bytes would
/// otherwise keep the old text. `library_rel` is data-dir-relative.
pub fn purge_parse_artifacts(data_dir: &std::path::Path, library_rel: &str) {
    for artifact in parse_artifacts(data_dir, library_rel) {
        let _ = if artifact.is_dir() {
            std::fs::remove_dir_all(artifact)
        } else {
            std::fs::remove_file(artifact)
        };
    }
}

/// The parse and embed artifacts of the file at `library_rel`
/// (`layout::derived`), whether or not they exist yet: what a purge deletes
/// and Drive hides. A spreadsheet's stem is its own name, and a `{name}.pdf`
/// beside it is a PDF-route leftover that counts too. An Office file's
/// converted PDF is not one: it is the file's viewable copy.
///
/// Outside the synced folders a `<stem>.md` may be the student's own note, so
/// there it counts only when it is provably the parser's: a parse record
/// beside it, or a spreadsheet's text (whose name carries the sheet's
/// extension).
pub fn parse_artifacts(data_dir: &std::path::Path, library_rel: &str) -> Vec<std::path::PathBuf> {
    artifact_slots(data_dir, library_rel)
        .into_iter()
        .filter(|(slot, _)| *slot != ArtifactSlot::Converted)
        .map(|(_, path)| path)
        .collect()
}

/// One file a library file carries with it: its converted PDF, one of its
/// parse artifacts, or a spreadsheet's PDF-route leftover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactSlot {
    Converted,
    Derived(crate::library::layout::DerivedKind),
    SheetPdf,
}

/// `parse_artifacts`, plus an Office file's converted PDF, each tagged with
/// its slot, so a move can ask `artifact_at` where the same slot lives for
/// the file's new path.
pub fn artifact_slots(
    data_dir: &std::path::Path,
    library_rel: &str,
) -> Vec<(ArtifactSlot, std::path::PathBuf)> {
    use crate::library::layout::{derived, zone_of, DerivedKind, Zone};
    let zone = zone_of(library_rel);
    if zone == Zone::Outside {
        return Vec::new();
    }
    let sheet = is_sheet(library_rel);
    let Some(pdf_rel) = parsed_pdf_rel(library_rel) else {
        return Vec::new();
    };
    let pdf = data_dir.join(&pdf_rel);
    let parsers_md = zone == Zone::Synced || sheet || derived(&pdf, DerivedKind::Pages).is_file();
    let mut slots: Vec<ArtifactSlot> = DerivedKind::ALL
        .into_iter()
        .filter(|kind| *kind != DerivedKind::Markdown || parsers_md)
        .map(ArtifactSlot::Derived)
        .collect();
    if sheet {
        slots.push(ArtifactSlot::SheetPdf);
    }
    if pdf_rel != library_rel && !sheet {
        slots.push(ArtifactSlot::Converted);
    }
    slots
        .into_iter()
        .filter_map(|slot| Some((slot, artifact_at(data_dir, library_rel, slot)?)))
        .collect()
}

/// Where `slot` lives for the file at `library_rel`; `None` when a file of
/// that name has no such artifact (a rename from `.docx` to `.txt`).
pub fn artifact_at(
    data_dir: &std::path::Path,
    library_rel: &str,
    slot: ArtifactSlot,
) -> Option<std::path::PathBuf> {
    use crate::library::layout::derived;
    let sheet = is_sheet(library_rel);
    match slot {
        ArtifactSlot::Converted => doc_pdf_rel(library_rel)
            .filter(|pdf| pdf != library_rel)
            .map(|pdf| data_dir.join(pdf)),
        ArtifactSlot::Derived(kind) => {
            parsed_pdf_rel(library_rel).map(|pdf| derived(&data_dir.join(pdf), kind))
        }
        ArtifactSlot::SheetPdf => sheet.then(|| data_dir.join(format!("{library_rel}.pdf"))),
    }
}

/// The PDF whose stem names a file's parse artifacts: the file itself, an
/// Office file's converted PDF, or for a spreadsheet `{name}.pdf`.
fn parsed_pdf_rel(library_rel: &str) -> Option<String> {
    doc_pdf_rel(library_rel).or_else(|| is_sheet(library_rel).then(|| format!("{library_rel}.pdf")))
}

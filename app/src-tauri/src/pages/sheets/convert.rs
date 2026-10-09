//! Converting a spreadsheet on disk to its `.md`.

use std::path::Path;

use crate::library::paths;
use crate::parse::{self, ParseError, ParsePage};

use super::csv::csv_section;
use super::{document, md_rel, sections};

/// Convert the spreadsheet at `rel` and write its `.md`, returning one page
/// per worksheet. Whatever text or PDF-route files were beside it go first,
/// so a workbook that no longer reads leaves no stale text behind.
pub fn convert(data_dir: &Path, rel: &str) -> Result<Vec<ParsePage>, ParseError> {
    let source = data_dir.join(rel);
    let bytes = std::fs::read(&source)
        .map_err(|e| ParseError::Io(format!("read {}: {e}", source.display())))?;
    paths::purge_parse_artifacts(data_dir, rel);

    let filename = rel.rsplit('/').next().unwrap_or(rel);
    let sections = if filename.to_ascii_lowercase().ends_with(".csv") {
        Ok(vec![csv_section(filename, &bytes)])
    } else {
        sections(&bytes)
    };
    let sections = sections.map_err(|detail| {
        // The UI keeps the sentence; calamine's reason goes to stderr.
        eprintln!("[oculus] spreadsheet unreadable: {rel}: {detail}");
        ParseError::Document {
            code: parse::SHEET_UNREADABLE.into(),
        }
    })?;
    let md = data_dir.join(md_rel(rel));
    let tmp = md.with_extension(format!(
        "md.tmp{}-{}",
        std::process::id(),
        crate::runtime::clock::now_nanos()
    ));
    crate::runtime::atomic_write::write(&md, &tmp, document(filename, &sections).as_bytes())
        .map_err(ParseError::Io)?;

    Ok(sections
        .into_iter()
        .enumerate()
        .map(|(i, markdown)| ParsePage {
            page_no: i as u32 + 1,
            markdown,
        })
        .collect())
}

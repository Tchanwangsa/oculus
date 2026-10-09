//! The artifacts beside a PDF, and the parsed document they record.
//!
//! The image directory's name is also the link prefix written into the markdown
//! (`![](<stem>_images/x.jpg)`), so both are computed here, never by a backend.

use super::{ImageStaging, ParseError, MODE, PARSER_VERSION};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub fn md_path(pdf: &Path) -> PathBuf {
    pdf.with_extension("md")
}

pub fn pages_path(pdf: &Path) -> PathBuf {
    pdf.with_extension("pages.json")
}

pub fn images_dir_for(pdf: &Path) -> PathBuf {
    let stem = pdf
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    pdf.with_file_name(format!("{stem}_images"))
}

/// The record beside this PDF, read back as the type that wrote it.
pub fn read_record(pdf: &Path) -> Option<ParseOutput> {
    serde_json::from_str(&fs::read_to_string(pages_path(pdf)).ok()?).ok()
}

/// `Some("quality")` when this PDF is parsed, `None` when it still needs it —
/// the record missing, unreadable, or naming another mode all mean parse it.
///
/// * Never inferred from the images directory or the `.md`: both survive a
///   crash that never wrote the record.
/// * No `parser_version` check: a version bump must not re-parse the library.
pub fn parse_mode(pdf: &Path) -> Option<&'static str> {
    let text = fs::read_to_string(pages_path(pdf)).ok()?;
    let record: serde_json::Value = serde_json::from_str(&text).ok()?;
    (record.get("mode").and_then(|v| v.as_str()) == Some(MODE)).then_some(MODE)
}

/// One page's markdown, keyed by its 1-based page number — the join key
/// retrieval rests on, which is why `ParseOutput::new` normalises it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsePage {
    pub page_no: u32,
    pub markdown: String,
}

/// Exactly the `.pages.json` on disk, plus one field that never goes there.
/// Field names are the wire format, and existing records must keep reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseOutput {
    /// The PDF's file name, not its path — the record travels with the folder.
    pub pdf: String,
    pub mode: String,
    pub parser_version: u32,
    pub page_count: u32,
    pub pages: Vec<ParsePage>,
    /// Omitted rather than null: a reader that sees the key can trust it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// For progress and logging; never written to the record.
    #[serde(skip)]
    pub image_count: u32,
}

impl ParseOutput {
    /// Exactly one entry per page `1..=page_count`, in order, `""` where a
    /// page yielded nothing — whatever order or gaps the backend returned.
    pub fn new(
        pdf: &Path,
        page_count: u32,
        pages: Vec<ParsePage>,
        backend: Option<String>,
        image_count: u32,
    ) -> Self {
        let mut slots = vec![String::new(); page_count as usize];
        for page in pages {
            if page.page_no >= 1 && page.page_no <= page_count {
                slots[(page.page_no - 1) as usize] = page.markdown;
            }
        }
        Self {
            pdf: pdf
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            mode: MODE.to_string(),
            parser_version: PARSER_VERSION,
            page_count,
            pages: slots
                .into_iter()
                .enumerate()
                .map(|(i, markdown)| ParsePage {
                    page_no: i as u32 + 1,
                    markdown,
                })
                .collect(),
            backend,
            image_count,
        }
    }

    /// The full-document markdown: pages joined by a blank line.
    pub fn document_markdown(&self) -> String {
        self.pages
            .iter()
            .map(|p| p.markdown.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Put the artifacts on disk. `.pages.json` is the only evidence a parse
    /// finished, so it lands last and atomically (temp file, fsync, rename).
    pub fn write(&self, pdf: &Path, images: ImageStaging) -> Result<(), ParseError> {
        images.commit()?;
        self.write_markdown(pdf)?;

        // serde_json leaves non-ASCII unescaped, as the existing records are.
        // The temp name is unique per write, so two writers never share one.
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let final_path = pages_path(pdf);
        let body = serde_json::to_vec(self)
            .map_err(|e| ParseError::Io(format!("encode {}: {e}", final_path.display())))?;
        let tmp = final_path.with_extension(format!(
            "json.tmp{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        crate::runtime::atomic_write::write(&final_path, &tmp, &body).map_err(ParseError::Io)
    }

    /// `<stem>.md`, derived from the record.
    fn write_markdown(&self, pdf: &Path) -> Result<(), ParseError> {
        let md = md_path(pdf);
        fs::write(&md, self.document_markdown())
            .map_err(|e| ParseError::Io(format!("write {}: {e}", md.display())))
    }

    /// Re-derive a missing `<stem>.md` from this record; true when written.
    /// The record is the evidence of the parse, so nothing is re-parsed.
    pub fn restore_markdown(&self, pdf: &Path) -> Result<bool, ParseError> {
        if md_path(pdf).is_file() {
            return Ok(false);
        }
        self.write_markdown(pdf).map(|()| true)
    }
}

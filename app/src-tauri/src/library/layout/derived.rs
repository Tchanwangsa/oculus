//! A PDF's parse and embed artifacts. Every one is named here, so where they
//! live is one function's decision: today, beside the PDF and named off its
//! stem.

use std::path::{Path, PathBuf};

/// What parsing and embedding derive from a PDF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivedKind {
    /// `<stem>.md`, the parsed text.
    Markdown,
    /// `<stem>.pages.json`, the parse record.
    Pages,
    /// `<stem>.emb.json`, the page vectors.
    Embeddings,
    /// `<stem>_images/`, the extracted pictures; its name is also the link
    /// prefix the markdown carries.
    Images,
}

impl DerivedKind {
    pub const ALL: [Self; 4] = [Self::Markdown, Self::Pages, Self::Embeddings, Self::Images];
}

/// Where `kind` lives for the PDF at `pdf` (absolute or relative alike). A
/// spreadsheet's text is `<name>.md`, the Markdown of its `<name>.pdf`.
pub fn derived(pdf: &Path, kind: DerivedKind) -> PathBuf {
    match kind {
        DerivedKind::Markdown => pdf.with_extension("md"),
        DerivedKind::Pages => pdf.with_extension("pages.json"),
        DerivedKind::Embeddings => pdf.with_extension("emb.json"),
        DerivedKind::Images => {
            let stem = pdf
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            pdf.with_file_name(format!("{stem}_images"))
        }
    }
}

/// `derived` for a data-dir-relative path, with `/` separators.
pub fn derived_rel(pdf_rel: &str, kind: DerivedKind) -> String {
    crate::library::paths::slash_path(derived(Path::new(pdf_rel), kind))
}

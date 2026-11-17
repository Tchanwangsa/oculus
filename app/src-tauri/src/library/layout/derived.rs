//! A PDF's parse and embed artifacts. Every one is named here, so where they
//! live is one function's decision. The parsed text and its pictures sit
//! beside the PDF, named off its stem, so the Markdown's image links stay
//! relative to it; the parse record and the page vectors are machine records
//! kept in the subject's hidden `.oculus/derived/`, under the PDF's own
//! folder path.

use std::path::{Component, Path, PathBuf};

use super::roots::COURSES_DIR;
use super::shape::CURRENT;
use super::zone::same_name;

/// What parsing and embedding derive from a PDF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivedKind {
    /// `<stem>.md`, the parsed text, beside the PDF.
    Markdown,
    /// `<stem>.pages.json`, the parse record.
    Pages,
    /// `<stem>.emb.json`, the page vectors.
    Embeddings,
    /// `<stem>_images/`, the extracted pictures, beside the PDF; its name is
    /// also the link prefix the markdown carries.
    Images,
}

impl DerivedKind {
    pub const ALL: [Self; 4] = [Self::Markdown, Self::Pages, Self::Embeddings, Self::Images];
}

/// Where `kind` lives for the PDF at `pdf` (absolute, or data-dir-relative).
/// A spreadsheet's text is `<name>.md`, the Markdown of its `<name>.pdf`. A
/// PDF outside any subject keeps every artifact beside it.
pub fn derived(pdf: &Path, kind: DerivedKind) -> PathBuf {
    let beside = match kind {
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
    };
    let (Some(hidden), DerivedKind::Pages | DerivedKind::Embeddings) = (CURRENT.derived, kind)
    else {
        return beside;
    };
    let (Some((root, folder)), Some(name)) = (subject_split(pdf), beside.file_name()) else {
        return beside;
    };
    root.join(hidden).join(folder).join(name)
}

/// `derived` for a data-dir-relative path, with `/` separators.
pub fn derived_rel(pdf_rel: &str, kind: DerivedKind) -> String {
    crate::library::paths::slash_path(derived(Path::new(pdf_rel), kind))
}

/// The subject folder `path` is in, and the folder inside it that holds
/// `path`. An absolute path is read against the data dir first, so a data
/// dir whose own path holds a `courses` folder is not misread; failing that
/// (a test's scratch library), the first `courses/<code>/` in it counts.
fn subject_split(path: &Path) -> Option<(PathBuf, PathBuf)> {
    if path.is_absolute() {
        let data_dir = crate::library::paths::data_dir();
        if let Ok(rel) = path.strip_prefix(&data_dir) {
            let (root, folder) = split_at_courses(rel, 0)?;
            return Some((data_dir.join(root), folder));
        }
    }
    let at = path.components().position(|c| is_courses(&c))?;
    split_at_courses(path, at)
}

fn is_courses(c: &Component) -> bool {
    matches!(c, Component::Normal(n) if n.to_str().is_some_and(|n| same_name(n, COURSES_DIR)))
}

/// Split `path` whose component `at` is `courses`: `(…/courses/<code>, the
/// folder between the subject root and the file)`.
fn split_at_courses(path: &Path, at: usize) -> Option<(PathBuf, PathBuf)> {
    let parts: Vec<Component> = path.components().collect();
    // `courses`, the code, then at least the file itself.
    let inside = parts.get(at + 1..).filter(|rest| rest.len() >= 2)?;
    if !is_courses(&parts[at]) || !inside.iter().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    let root: PathBuf = parts[..at + 2].iter().collect();
    let folder: PathBuf = parts[at + 2..parts.len() - 1].iter().collect();
    Some((root, folder))
}

/// True when the `.md` at `md_rel` (data-dir-relative) is text derived from
/// a file beside it rather than a note: a spreadsheet's text, or a parsed
/// PDF's (its parse record exists). Outside the synced folders the two look
/// alike, so listings of the student's notes ask this.
pub fn is_derived_text(data_dir: &Path, md_rel: &str) -> bool {
    let Some(stem) = md_rel.strip_suffix(".md") else {
        return false;
    };
    if crate::library::paths::is_sheet(stem) && data_dir.join(stem).is_file() {
        return true;
    }
    [".pdf", ".PDF"].iter().any(|ext| {
        let pdf = data_dir.join(format!("{stem}{ext}"));
        derived(&pdf, DerivedKind::Pages).is_file()
    })
}

//! Walks over the old tree: the debris to delete first, and in each subject
//! the markdown to re-point and the parse records to file under
//! `.oculus/derived/`.

use std::path::Path;

use super::{join, list, Builder, Entry};
use crate::library::layout::{self, COURSES_DIR};
use crate::library::migrate::journal::{Kind, Op};

/// An agent tool's scratch folder (`.claude/.cc-writes`).
const AGENT_SCRATCH: &str = ".claude";

/// What a dot-folder's name carries when an interrupted parse left it: an
/// image staging folder or a quality run's scratch.
const STAGING_MARKS: [&str; 2] = ["-staging-", "-quality-"];

/// The suffixes of the two parse records the move files under
/// `.oculus/derived/`, each with what replaces it to name the PDF.
pub(super) const RECORD_SUFFIXES: [&str; 2] = [".pages.json", ".emb.json"];

fn is_debris_name(name: &str) -> bool {
    name == AGENT_SCRATCH
        || (name.starts_with('.') && STAGING_MARKS.iter().any(|m| name.contains(m)))
}

/// True when no file sits anywhere under `dir`: only folders.
pub(in crate::library::migrate) fn holds_no_file(dir: &Path) -> bool {
    list(dir).into_iter().all(|(name, kind)| match kind {
        Entry::Dir => holds_no_file(&dir.join(name)),
        Entry::File | Entry::Symlink => false,
    })
}

/// Agent scratch and parse staging folders under `courses/` and the old
/// recordings folder, deleted first when they hold no file. One that holds a
/// file is moved with its folder like anything else.
pub(super) fn debris(b: &mut Builder) {
    for root in [COURSES_DIR, layout::legacy_recordings_root()] {
        sweep(b, root);
    }
}

fn sweep(b: &mut Builder, rel: &str) {
    for (name, kind) in list(&b.data_dir.join(rel)) {
        if kind != Entry::Dir {
            continue;
        }
        let child = join(rel, &name);
        if is_debris_name(&name) && holds_no_file(&b.data_dir.join(&child)) {
            b.debris.insert(child.clone());
            b.step(Kind::Debris, Op::RemoveEmptyTree { path: child });
        } else {
            sweep(b, &child);
        }
    }
}

/// What one walk of a subject's old folders found, by old path.
#[derive(Default)]
pub(super) struct Found {
    pub markdown: Vec<String>,
    pub records: Vec<String>,
}

/// Every markdown file and parse record under `rels` (old paths, files or
/// folders), symlinks and debris skipped.
pub(super) fn find(b: &Builder, rels: &[String]) -> Found {
    let mut found = Found::default();
    for rel in rels {
        match std::fs::symlink_metadata(b.data_dir.join(rel)) {
            Ok(meta) if meta.is_dir() => collect(b, rel, &mut found),
            Ok(meta) if meta.is_file() => classify(rel, &mut found),
            _ => {}
        }
    }
    found
}

fn collect(b: &Builder, rel: &str, found: &mut Found) {
    for (name, kind) in list(&b.data_dir.join(rel)) {
        let child = join(rel, &name);
        match kind {
            Entry::Dir if !b.debris.contains(&child) => collect(b, &child, found),
            Entry::File => classify(&child, found),
            _ => {}
        }
    }
}

fn classify(rel: &str, found: &mut Found) {
    let name = super::base(rel);
    if name.starts_with('.') {
        return;
    }
    if RECORD_SUFFIXES
        .iter()
        .any(|s| name.ends_with(s) && name.len() > s.len())
    {
        found.records.push(rel.to_string());
    } else if name.to_ascii_lowercase().ends_with(".md") {
        found.markdown.push(rel.to_string());
    }
}

/// Where a parse record at `at` (wherever the folder moves put it) is filed:
/// `layout::derived` for the PDF its name is made from.
pub(super) fn record_home(at: &str) -> Option<String> {
    let (suffix, kind) = [
        (RECORD_SUFFIXES[0], layout::DerivedKind::Pages),
        (RECORD_SUFFIXES[1], layout::DerivedKind::Embeddings),
    ]
    .into_iter()
    .find(|(s, _)| at.ends_with(s))?;
    let pdf = format!("{}.pdf", at.strip_suffix(suffix)?);
    Some(layout::derived_rel(&pdf, kind))
}

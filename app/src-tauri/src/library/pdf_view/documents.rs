//! Library paths the viewer may read, and the cache of opened documents.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use hayro::hayro_syntax::page::Page;
use hayro::hayro_syntax::{LoadPdfError, Pdf};

use super::wire::{OpenedPdf, PageSize};

/// The library roots the viewer may read: the asset-protocol scope in
/// tauri.conf.json.
const ROOTS: [&str; 3] = ["courses", "lectures", "agents"];
const OPEN_DOCS: usize = 4;

/// `relative` under `root`, if it names something inside one of [`ROOTS`].
pub(super) fn resolve_in(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    let mut parts = path.components();
    let in_root = matches!(
        parts.next(),
        Some(Component::Normal(first)) if first.to_str().is_some_and(|first| ROOTS.contains(&first))
    );
    if !in_root || !parts.all(|part| matches!(part, Component::Normal(_))) {
        return Err("outside-library".into());
    }
    Ok(root.join(path))
}

pub(super) struct OpenDoc {
    pub(super) path: PathBuf,
    /// Modification time and length: a rewritten file is reopened.
    stamp: (SystemTime, u64),
    pdf: Arc<Pdf>,
}

/// Most recently used last.
static DOCS: Mutex<Vec<OpenDoc>> = Mutex::new(Vec::new());

pub(super) fn docs() -> MutexGuard<'static, Vec<OpenDoc>> {
    DOCS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The document at `relative`, from the cache when its file is unchanged.
/// Parsing happens outside the lock, so one slow file holds up no other.
pub(super) fn open_at(root: &Path, relative: &str) -> Result<Arc<Pdf>, String> {
    let path = resolve_in(root, relative)?;
    let meta = std::fs::metadata(&path).map_err(|_| "not-found".to_string())?;
    if !meta.is_file() {
        return Err("not-found".into());
    }
    let stamp = (
        meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        meta.len(),
    );
    {
        let mut docs = docs();
        if let Some(at) = docs
            .iter()
            .position(|doc| doc.path == path && doc.stamp == stamp)
        {
            let doc = docs.remove(at);
            let pdf = doc.pdf.clone();
            docs.push(doc);
            return Ok(pdf);
        }
    }
    let bytes = std::fs::read(&path).map_err(|_| "not-found".to_string())?;
    let pdf = guarded(|| Pdf::new(Arc::new(bytes)))?.map_err(|error| {
        match error {
            LoadPdfError::Decryption(_) => "encrypted",
            LoadPdfError::Invalid => "invalid",
        }
        .to_string()
    })?;
    let pdf = Arc::new(pdf);
    let mut docs = docs();
    docs.retain(|doc| doc.path != path);
    docs.push(OpenDoc {
        path,
        stamp,
        pdf: pdf.clone(),
    });
    if docs.len() > OPEN_DOCS {
        docs.remove(0);
    }
    Ok(pdf)
}

/// Runs a hayro call, turning a panic inside it into "render-failed".
pub(super) fn guarded<T>(work: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(work)).map_err(|_| "render-failed".to_string())
}

pub(super) fn page_sizes(pdf: &Pdf) -> OpenedPdf {
    let pages = pdf
        .pages()
        .iter()
        .map(|page| {
            let (width, height) = page.render_dimensions();
            PageSize { width, height }
        })
        .collect();
    OpenedPdf { pages }
}

pub(super) fn page_of(pdf: &Pdf, page: u32) -> Result<&Page<'_>, String> {
    (page as usize)
        .checked_sub(1)
        .and_then(|index| pdf.pages().get(index))
        .ok_or_else(|| "page-out-of-range".to_string())
}

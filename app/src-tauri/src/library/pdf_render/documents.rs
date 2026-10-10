//! Opening documents: uncached, or through the viewer's small LRU keyed by
//! path and modification stamp, with every hayro call under `catch_unwind`.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use hayro::hayro_syntax::{LoadPdfError, Pdf};

/// Documents [`open_cached`] keeps open.
const OPEN_DOCS: usize = 4;

/// hayro panicked inside a guarded call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panicked;

/// Why a document did not open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// The file could not be read; the I/O error's text.
    Read(String),
    /// Locked by a password other than the empty one.
    Encrypted,
    /// Not a PDF hayro can parse.
    Invalid,
    /// hayro panicked while parsing it.
    Panicked,
}

/// Runs a hayro call, turning a panic inside it into [`Panicked`].
pub fn guarded<T>(work: impl FnOnce() -> T) -> Result<T, Panicked> {
    catch_unwind(AssertUnwindSafe(work)).map_err(|_| Panicked)
}

/// The document at `path`, parsed afresh.
pub fn open(path: &Path) -> Result<Pdf, OpenError> {
    let bytes = std::fs::read(path).map_err(|error| OpenError::Read(error.to_string()))?;
    match guarded(|| Pdf::new(Arc::new(bytes))) {
        Ok(Ok(pdf)) => Ok(pdf),
        Ok(Err(LoadPdfError::Decryption(_))) => Err(OpenError::Encrypted),
        Ok(Err(LoadPdfError::Invalid)) => Err(OpenError::Invalid),
        Err(Panicked) => Err(OpenError::Panicked),
    }
}

/// How many pages the document at `path` has, as hayro reads its page tree —
/// the count `page_no` is joined on (the parse record's and the embedder's).
/// Zero is a count, not an error; callers decide.
pub fn page_count(path: &Path) -> Result<u32, OpenError> {
    let pdf = open(path)?;
    guarded(|| pdf.pages().len() as u32).map_err(|_| OpenError::Panicked)
}

struct OpenDoc {
    path: PathBuf,
    /// Modification time and length: a rewritten file is reopened.
    stamp: (SystemTime, u64),
    pdf: Arc<Pdf>,
}

/// Most recently used last.
static DOCS: Mutex<Vec<OpenDoc>> = Mutex::new(Vec::new());

fn docs() -> MutexGuard<'static, Vec<OpenDoc>> {
    crate::providers::ratelimit::hold(&DOCS)
}

/// The document at `path`, from the cache when its file is unchanged. Parsing
/// happens outside the lock, so one slow file holds up no other.
pub fn open_cached(path: &Path) -> Result<Arc<Pdf>, OpenError> {
    let meta = std::fs::metadata(path).map_err(|error| OpenError::Read(error.to_string()))?;
    if !meta.is_file() {
        return Err(OpenError::Read("not a file".into()));
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
    let pdf = Arc::new(open(path)?);
    let mut docs = docs();
    docs.retain(|doc| doc.path != path);
    docs.push(OpenDoc {
        path: path.to_path_buf(),
        stamp,
        pdf: pdf.clone(),
    });
    if docs.len() > OPEN_DOCS {
        docs.remove(0);
    }
    Ok(pdf)
}

/// Drops `path` from [`open_cached`]'s cache.
pub fn forget(path: &Path) {
    docs().retain(|doc| doc.path != path);
}

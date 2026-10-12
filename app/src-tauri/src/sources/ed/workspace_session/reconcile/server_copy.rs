//! Where a conflict puts Ed's version of a file: beside it, `.ed-server`
//! before the last extension so the copy keeps its language (`a.py` →
//! `a.ed-server.py`, `x.tar.gz` → `x.tar.ed-server.gz`), or after a name
//! with none (`Makefile` → `Makefile.ed-server`). Names of that shape are
//! reserved: reconcile never uploads, records or downloads one.

const MARK: &str = ".ed-server";

/// The server copy's path for workspace file `path`.
pub fn name(path: &str) -> String {
    let (dir, file) = split(path);
    let copy = match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => {
            format!("{stem}{MARK}.{ext}")
        }
        _ => format!("{file}{MARK}"),
    };
    format!("{dir}{copy}")
}

/// The file a server copy belongs to, `None` when `path` is not one.
pub fn original(path: &str) -> Option<String> {
    let (dir, file) = split(path);
    let bare = match file.strip_suffix(MARK) {
        Some(stem) => stem.to_string(),
        None => {
            let (stem, ext) = file.rsplit_once('.')?;
            format!("{}.{ext}", stem.strip_suffix(MARK)?)
        }
    };
    (!bare.is_empty() && name(&format!("{dir}{bare}")) == path).then(|| format!("{dir}{bare}"))
}

pub fn is_copy(path: &str) -> bool {
    original(path).is_some()
}

/// `dir/` (with its slash, or empty) and the file name.
fn split(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(i) => path.split_at(i + 1),
        None => ("", path),
    }
}

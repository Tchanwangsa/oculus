//! Which download a `Finished` event is for. WebKit's names only the URL the
//! download was requested with; WebView2's also names the file it wrote.

use std::path::Path;

/// A download that has not finished, in the order they started.
pub(super) struct Pending<'a> {
    pub id: u32,
    pub url: &'a str,
    pub staged: &'a Path,
}

/// A reported path names its download exactly. Without one, or when it names
/// none of them, the oldest unfinished download of `url` is the one, so two
/// downloads of one URL finish first in, first out.
pub(super) fn match_finished(
    pending: &[Pending<'_>],
    url: &str,
    path: Option<&Path>,
) -> Option<u32> {
    if let Some(path) = path {
        if let Some(found) = pending.iter().find(|p| same_file(p.staged, path)) {
            return Some(found.id);
        }
    }
    pending.iter().find(|p| p.url == url).map(|p| p.id)
}

/// WebView2 may hand the path back in its `\\?\` verbatim form.
fn same_file(a: &Path, b: &Path) -> bool {
    let plain = |p: &Path| {
        let s = p.to_string_lossy();
        s.strip_prefix(r"\\?\")
            .map(str::to_string)
            .unwrap_or_else(|| s.to_string())
    };
    a == b || plain(a) == plain(b)
}

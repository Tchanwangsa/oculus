//! Which downloads a `Finished` event settles.
//!
//! A staged download's `Finished` names the path the hook gave it: always on
//! WebKit (change 3 in `vendor/wry/VENDORED.md`), on success only on
//! WebView2. A path names its download exactly, so two downloads of one URL,
//! a `blob:` saved twice or a megabyte `data:` URL need nothing special.
//!
//! With no path, a failure on WebKit is not ours and is dropped: the hook
//! refused it, or it is the orphan a cross-origin `<a download>` redirect
//! fails with before any `Requested`. On WebView2 it is a staged download's
//! failure, and a success anywhere means the wry change is not in effect.
//! Those two are matched by URL within their page, never by picking one of
//! several: each is owed to its URL until as many have ended as are
//! unfinished, and the group then settles together, failed if any failed.

use std::path::Path;

/// A download that has not finished.
pub(super) struct Pending<'a> {
    pub id: u32,
    pub page: u32,
    pub url_key: u64,
    pub staged: &'a Path,
}

/// What `Finished` reported, in the page it arrived in.
#[derive(Clone, Copy)]
pub(super) struct Ended<'a> {
    pub page: u32,
    pub url_key: u64,
    pub path: Option<&'a Path>,
    pub success: bool,
}

/// A pathless `Finished` matched by URL, waiting for the rest of its group.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Owed {
    page: u32,
    url_key: u64,
    success: bool,
}

/// Whether a pathless failure can be a staged download's: wry reads
/// WebView2's result path only on success, and always passes WebKit's.
pub(super) const PATHLESS_FAILURES_ARE_OURS: bool = cfg!(windows);

/// The downloads `ended` settles now, each with whether it succeeded.
/// `owed` carries pathless events between calls.
pub(super) fn settle(
    pending: &[Pending<'_>],
    owed: &mut Vec<Owed>,
    ended: Ended<'_>,
    pathless_failures_are_ours: bool,
) -> Vec<(u32, bool)> {
    let mut settled = Vec::new();
    match ended.path {
        Some(path) => {
            if let Some(found) = pending.iter().find(|p| same_file(p.staged, path)) {
                settled.push((found.id, ended.success));
            }
        }
        None if ended.success || pathless_failures_are_ours => owed.push(Owed {
            page: ended.page,
            url_key: ended.url_key,
            success: ended.success,
        }),
        None => {}
    }
    let by_path = settled.clone();
    settle_owed(pending, owed, &by_path, &mut settled);
    settled
}

/// Settles every URL group with as many owed events as unfinished
/// downloads, leaving out the ones in `done`; drops owed events whose group
/// has nothing left to settle.
fn settle_owed(
    pending: &[Pending<'_>],
    owed: &mut Vec<Owed>,
    done: &[(u32, bool)],
    out: &mut Vec<(u32, bool)>,
) {
    let mut keys: Vec<(u32, u64)> = owed.iter().map(|o| (o.page, o.url_key)).collect();
    keys.sort_unstable();
    keys.dedup();
    for (page, url_key) in keys {
        let group: Vec<u32> = pending
            .iter()
            .filter(|p| p.page == page && p.url_key == url_key)
            .map(|p| p.id)
            .filter(|id| !done.iter().any(|(d, _)| d == id))
            .collect();
        let events: Vec<&Owed> = owed
            .iter()
            .filter(|o| o.page == page && o.url_key == url_key)
            .collect();
        if !group.is_empty() && events.len() < group.len() {
            continue;
        }
        let success = events.iter().all(|o| o.success);
        out.extend(group.into_iter().map(|id| (id, success)));
        owed.retain(|o| o.page != page || o.url_key != url_key);
    }
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

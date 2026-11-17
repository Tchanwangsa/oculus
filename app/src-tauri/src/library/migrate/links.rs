//! Paths inside markdown: relative links and absolute `asset://` picture
//! links, re-pointed after the move.
//!
//! A relative link is read from the file's old place and its target followed
//! through the rules; when that is not what the link names from the file's
//! new place (exact case), it is rewritten with `link_between`, as the sync
//! now writes links. So a link to a sibling renamed on a clash follows it to
//! its new name, even when a file with the old name now sits beside the note.
//! A re-pointed link no longer names anything from the old place, so a
//! second pass leaves it and a run cut short can rewrite again. An
//! `asset://` link holds an absolute path from wherever the library was; its
//! `courses/…` tail is followed through the rules and joined to this data dir.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::rules::Rules;
use crate::library::layout::{self, COURSES_DIR};

/// `](target)` or `](<target>)`, a link's or a picture's.
static LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\]\(\s*(<[^>\n]*>|[^)\s]+)").expect("valid"));

/// What `convertFileSrc` makes of an absolute path: macOS's scheme, and
/// Windows's.
static ASSET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(asset://localhost/|http://asset\.localhost/)([^)\s"'<>]+)"#).expect("valid")
});

pub struct Linker<'a> {
    data_dir: &'a Path,
    rules: &'a Rules,
    /// Names in each folder (data-dir-relative), exact case.
    listings: RefCell<HashMap<String, HashSet<String>>>,
}

impl<'a> Linker<'a> {
    pub fn new(data_dir: &'a Path, rules: &'a Rules) -> Self {
        Self {
            data_dir,
            rules,
            listings: RefCell::new(HashMap::new()),
        }
    }

    /// `text` of the markdown file that was at `old` and is now at `new`
    /// (data-dir-relative), with its links re-pointed, and how many changed.
    pub fn relink(&self, text: &str, old: &str, new: &str) -> (String, usize) {
        let (text, assets) = self.assets(text);
        let (text, links) = self.relative(&text, old, new);
        (text, assets + links)
    }

    fn relative(&self, text: &str, old: &str, new: &str) -> (String, usize) {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        let mut changed = 0;
        for caps in LINK.captures_iter(text) {
            let m = caps.get(1).expect("one group");
            let Some(link) = self.repoint(m.as_str(), old, new) else {
                continue;
            };
            out.push_str(&text[last..m.start()]);
            out.push_str(&link);
            last = m.end();
            changed += 1;
        }
        out.push_str(&text[last..]);
        (out, changed)
    }

    /// The rewritten target of one link, or `None` to leave it.
    fn repoint(&self, raw: &str, old: &str, new: &str) -> Option<String> {
        let angled = raw.starts_with('<');
        let target = raw.trim_start_matches('<').trim_end_matches('>');
        let first = target.split('/').next().unwrap_or_default();
        if target.is_empty()
            || target.starts_with(['#', '/', '\\'])
            || target.contains("://")
            || first.contains(':')
        {
            return None;
        }
        let cut = target.find(['#', '?']).unwrap_or(target.len());
        let (path, suffix) = target.split_at(cut);
        let encoded = path.contains('%');
        let path = if encoded {
            percent_decode(path)?
        } else {
            path.to_string()
        };
        let here = normalize(parent(new), &path);
        let reads_here = here.as_deref().is_some_and(|p| self.exists_exact(p));
        let was = normalize(parent(old), &path)?;
        // The target is what the link named from the file's old place,
        // followed through the rules. One not on disk is followed only when a
        // rule moved its folder: a page or video the sync has not fetched
        // yet, linked where it will land (a video in `Media/`). A link whose
        // old reading names nothing but whose new one does was re-pointed by
        // an earlier pass.
        let now = match self.rules.map(&was) {
            Some(now) if self.exists_exact(&now) => now,
            None if self.exists_exact(&was) => was,
            _ if reads_here => return None,
            Some(now) => pending_video(&was).unwrap_or(now),
            None => return None,
        };
        if here.as_deref() == Some(now.as_str()) {
            return None;
        }
        let link = layout::link_between(new, &now);
        let link = if encoded { encode(&link, true) } else { link };
        let link = if angled || link.contains([' ', '(', ')', '<', '>']) {
            format!("<{link}>")
        } else {
            link
        };
        let link = format!("{link}{suffix}");
        (link != raw).then_some(link)
    }

    fn assets(&self, text: &str) -> (String, usize) {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        let mut changed = 0;
        for caps in ASSET.captures_iter(text) {
            let m = caps.get(2).expect("two groups");
            let Some(path) = percent_decode(m.as_str()) else {
                continue;
            };
            let Some(now) = self.moved_absolute(&path) else {
                continue;
            };
            out.push_str(&text[last..m.start()]);
            out.push_str(&encode(&now, false));
            last = m.end();
            changed += 1;
        }
        out.push_str(&text[last..]);
        (out, changed)
    }

    /// An absolute path into some library's `courses/`, followed through the
    /// rules into this data dir; `None` when no rule moved it.
    fn moved_absolute(&self, path: &str) -> Option<String> {
        let slashed = path.replace('\\', "/");
        let marker = format!("/{COURSES_DIR}/");
        slashed.match_indices(&marker).find_map(|(at, _)| {
            let moved = self.rules.map(&slashed[at + 1..])?;
            Some(crate::library::paths::slash_path(self.data_dir.join(moved)))
        })
    }

    /// True when `rel` names something on disk spelled exactly so: a
    /// case-insensitive volume would find `files/` for `Files/`, and the
    /// app's own lookups are exact.
    pub fn exists_exact(&self, rel: &str) -> bool {
        let mut dir = String::new();
        for part in rel.split('/') {
            if !self.listing(&dir).contains(part) {
                return false;
            }
            dir = if dir.is_empty() {
                part.to_string()
            } else {
                format!("{dir}/{part}")
            };
        }
        true
    }

    fn listing(&self, dir: &str) -> std::cell::Ref<'_, HashSet<String>> {
        if !self.listings.borrow().contains_key(dir) {
            let names = std::fs::read_dir(self.data_dir.join(dir))
                .map(|read| {
                    read.flatten()
                        .filter_map(|e| e.file_name().into_string().ok())
                        .collect()
                })
                .unwrap_or_default();
            self.listings.borrow_mut().insert(dir.to_string(), names);
        }
        std::cell::Ref::map(self.listings.borrow(), |l| &l[dir])
    }
}

/// Where a module video not downloaded yet lands, for a link into the old
/// `Files/`.
fn pending_video(was: &str) -> Option<String> {
    let (code, inner) = layout::split_course(was)?;
    let (dir, name) = inner.rsplit_once('/')?;
    (dir == layout::legacy_files_dir() && super::db::is_video(name))
        .then(|| layout::in_course(code, &layout::synced(layout::SyncedKind::Media, name)))
}

fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// `rel` read from the folder `base` (both data-dir-relative); `None` when
/// it climbs out of the data dir.
pub(super) fn normalize(base: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

pub(super) fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `encodeURIComponent`, which `convertFileSrc` applies to the whole path;
/// `keep_slash` leaves `/` for a relative link.
pub(super) fn encode(s: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let plain =
            b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) || (keep_slash && b == b'/');
        if plain {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

//! Reading one folder (or everything below it) off the disk.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::Metadata;
use std::path::Path;
use std::time::UNIX_EPOCH;

use super::entries::{zone_name, FileEntry, FolderEntry, FolderListing, RecordingEntry};
use super::recordings::Recording;
use crate::library::layout::{self, Zone};
use crate::library::paths::{doc_pdf_rel, parse_artifacts, slash_path};

/// A recursive listing stops this many folders down.
const MAX_DEPTH: usize = 32;

/// A name no folder needs to hold, for asking whether a new entry there
/// would be the student's own.
const PROBE: &str = "probe";

/// `rel`'s contents: the Drive root (`""`, whose folders are the subjects),
/// a subject's folder or any folder inside one. `recursive` lists every
/// folder and file below it instead, flat, with no recordings (at the root,
/// every subject's). Refuses a
/// path outside the subjects, in hidden app state, under a dot-name or
/// through a symlink.
pub fn list_folder_at(
    data_dir: &Path,
    rel: &str,
    recursive: bool,
    recordings: &[Recording],
) -> Result<FolderListing, String> {
    if rel.is_empty() {
        let mut listing = root_listing(data_dir);
        if recursive {
            let subjects: Vec<String> = listing
                .folders
                .iter()
                .map(|f| f.relative_path.clone())
                .collect();
            for subject in subjects {
                walk(data_dir, &subject, 0, &mut listing);
            }
            listing
                .folders
                .sort_by(|a, b| natural_cmp(&a.name, &b.name));
            listing.files.sort_by(|a, b| natural_cmp(&a.name, &b.name));
        }
        return Ok(listing);
    }
    check_folder(data_dir, rel)?;
    let zone = layout::zone_of(rel);
    let mut listing = FolderListing {
        rel: rel.to_string(),
        zone: zone_name(zone),
        writable: writable(rel),
        folders: Vec::new(),
        files: Vec::new(),
        recordings: Vec::new(),
    };
    if recursive {
        walk(data_dir, rel, 0, &mut listing);
    } else {
        let here: Vec<&Recording> = recordings
            .iter()
            .filter(|r| parent_rel(&r.folder_rel) == rel)
            .collect();
        let taken: HashSet<&str> = here.iter().map(|r| r.folder_rel.as_str()).collect();
        for child in visible_children(data_dir, rel) {
            if child.dir && taken.contains(child.rel.as_str()) {
                continue;
            }
            push_child(data_dir, child, &mut listing, true);
        }
        listing.recordings = here
            .into_iter()
            .map(|r| RecordingEntry {
                lecture_id: r.lecture_id.clone(),
                subject_id: r.subject_id,
                relative_path: r.folder_rel.clone(),
                title: r.title.clone(),
                date: r.date.clone(),
            })
            .collect();
        listing.recordings.sort_by(|a, b| a.date.cmp(&b.date));
    }
    listing
        .folders
        .sort_by(|a, b| natural_cmp(&a.name, &b.name));
    listing.files.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    Ok(listing)
}

/// The folder holding every subject, data-dir-relative.
fn subjects_rel() -> String {
    slash_path(layout::courses_dir(Path::new("")))
}

/// Every subject folder; loose files beside them belong to no subject.
fn root_listing(data_dir: &Path) -> FolderListing {
    let mut listing = FolderListing {
        rel: String::new(),
        zone: zone_name(Zone::Synced),
        writable: false,
        folders: Vec::new(),
        files: Vec::new(),
        recordings: Vec::new(),
    };
    for child in visible_children(data_dir, &subjects_rel()) {
        if child.dir {
            push_child(data_dir, child, &mut listing, true);
        }
    }
    listing
        .folders
        .sort_by(|a, b| natural_cmp(&a.name, &b.name));
    listing
}

/// A subject's folder or a folder inside one, with no dot-name on the way,
/// in a zone the student sees, and no symlink leading out of the data dir.
fn check_folder(data_dir: &Path, rel: &str) -> Result<(), String> {
    let refuse = || format!("{rel} is not a folder Drive shows");
    if layout::split_course(&format!("{rel}/{PROBE}")).is_none()
        || rel.split('/').any(|part| part.starts_with('.'))
        || !matches!(layout::zone_of(rel), Zone::Synced | Zone::Own)
    {
        return Err(refuse());
    }
    let path = layout::resolve(data_dir, rel)?;
    if !path.is_dir() {
        return Err(format!("{rel} is not a folder"));
    }
    // `resolve` lets a symlink through while it stays inside the data dir;
    // a listing never follows one.
    let mut walked = data_dir.to_path_buf();
    for part in rel.split('/') {
        walked.push(part);
        if std::fs::symlink_metadata(&walked).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(refuse());
        }
    }
    Ok(())
}

/// A file or folder made at `rel/<name>` would be the student's own.
fn writable(rel: &str) -> bool {
    !rel.is_empty() && layout::own_path_ok(&format!("{rel}/{PROBE}"))
}

fn parent_rel(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(parent, _)| parent)
}

struct Child {
    name: String,
    rel: String,
    dir: bool,
    meta: Metadata,
}

/// The entries of `rel` a listing shows: no dot-names, no symlinks, nothing
/// outside the student's view, and no file's parse artifacts or converted
/// PDF.
fn visible_children(data_dir: &Path, rel: &str) -> Vec<Child> {
    let dir = data_dir.join(rel);
    let Ok(read) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let children: Vec<Child> = read
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if name.starts_with('.') {
                return None;
            }
            let meta = entry.path().symlink_metadata().ok()?;
            if meta.file_type().is_symlink() {
                return None;
            }
            let rel = format!("{rel}/{name}");
            if !matches!(layout::zone_of(&rel), Zone::Synced | Zone::Own) {
                return None;
            }
            Some(Child {
                name,
                rel,
                dir: meta.is_dir(),
                meta,
            })
        })
        .collect();
    let derived = derived_names(data_dir, &dir, &children);
    children
        .into_iter()
        .filter(|c| !derived.contains(&c.name))
        .collect()
}

/// Names in `dir` that are another entry's parse artifacts or converted PDF.
fn derived_names(data_dir: &Path, dir: &Path, children: &[Child]) -> HashSet<String> {
    let mut names = HashSet::new();
    for child in children.iter().filter(|c| !c.dir) {
        let converted = doc_pdf_rel(&child.rel)
            .filter(|pdf| *pdf != child.rel)
            .map(|pdf| data_dir.join(pdf));
        for path in parse_artifacts(data_dir, &child.rel)
            .into_iter()
            .chain(converted)
        {
            if path.parent() == Some(dir) {
                if let Some(name) = path.file_name() {
                    names.insert(name.to_string_lossy().into_owned());
                }
            }
        }
    }
    names
}

fn push_child(data_dir: &Path, child: Child, listing: &mut FolderListing, count: bool) {
    let zone = zone_name(layout::zone_of(&child.rel));
    let modified = modified_ms(&child.meta);
    if child.dir {
        let child_count = if count {
            visible_children(data_dir, &child.rel).len() as u32
        } else {
            0
        };
        listing.folders.push(FolderEntry {
            writable: writable(&child.rel),
            editable: layout::own_path_ok(&child.rel),
            relative_path: child.rel,
            name: child.name,
            zone,
            child_count,
            modified,
        });
    } else {
        listing.files.push(FileEntry {
            editable: layout::own_path_ok(&child.rel),
            relative_path: child.rel,
            name: child.name,
            zone,
            size: child.meta.len(),
            modified,
        });
    }
}

/// Every folder and file below `rel`, each folder counted as it is read;
/// returns how many entries `rel` itself holds.
fn walk(data_dir: &Path, rel: &str, depth: usize, listing: &mut FolderListing) -> u32 {
    let children = visible_children(data_dir, rel);
    let count = children.len() as u32;
    if depth >= MAX_DEPTH {
        return count;
    }
    for child in children {
        if child.dir {
            let below = child.rel.clone();
            let at = listing.folders.len();
            push_child(data_dir, child, listing, false);
            listing.folders[at].child_count = walk(data_dir, &below, depth + 1, listing);
        } else {
            push_child(data_dir, child, listing, false);
        }
    }
    count
}

fn modified_ms(meta: &Metadata) -> Option<u64> {
    let since = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    u64::try_from(since.as_millis()).ok()
}

/// Names as Finder orders them: case-insensitive, runs of digits by value
/// (`Week 2` before `Week 10`).
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(p), Some(q)) if p.is_ascii_digit() && q.is_ascii_digit() => {
                let m = digits(&mut x);
                let n = digits(&mut y);
                let order = m
                    .trim_start_matches('0')
                    .len()
                    .cmp(&n.trim_start_matches('0').len())
                    .then_with(|| m.trim_start_matches('0').cmp(n.trim_start_matches('0')));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(p), Some(q)) => {
                let order = p.to_lowercase().cmp(q.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                x.next();
                y.next();
            }
        }
    }
}

fn digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut run = String::new();
    while let Some(c) = chars.peek().copied().filter(char::is_ascii_digit) {
        run.push(c);
        chars.next();
    }
    run
}

/// `path` relative to the data dir, `/`-separated; `None` outside it.
pub fn data_rel(data_dir: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(data_dir).ok().map(slash_path)
}

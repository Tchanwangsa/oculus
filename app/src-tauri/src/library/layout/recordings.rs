//! Lecture recordings: one folder per recording, holding its streams,
//! transcript and chapter frames, in its subject's synced `Recordings/`.
//!
//! A folder is named `<YYYY-MM-DD>-<slug>` from the lecture's date and title
//! when it is first needed, and the lecture's row keeps that name
//! (`lectures.folder_rel`), so a later title change never moves it.
//! `db::store::recording_folder` makes the choice; this module spells it.

use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

use super::roots::{courses_dir, in_course};
use super::shape::{Recordings, Shape, CURRENT};
use super::zone::{same_name, segments, Zone};

/// A slug's longest run, before any `-2` suffix.
const SLUG_MAX: usize = 60;

/// The slug of a title with no words left once the subject code is gone.
const UNTITLED: &str = "lecture";

/// `courses/<code>/Course/Recordings`, data-dir-relative: where a subject's
/// recordings go.
pub fn recordings_rel(code: &str) -> String {
    CURRENT.recordings_rel(code)
}

/// A recording's folder, data-dir-relative, from its subject and name.
pub fn recording_rel(code: &str, name: &str) -> String {
    format!("{}/{name}", recordings_rel(code))
}

/// The folder a new recording takes: `recording_name`, with `-2`, `-3`… when
/// `taken` (which compares ignoring case, as the filesystem does) says the
/// plain one is someone else's.
pub fn new_recording_rel(
    code: &str,
    date: &str,
    title: &str,
    taken: impl Fn(&str) -> bool,
) -> String {
    let base = recording_name(date, title, code);
    std::iter::once(base.clone())
        .chain((2..).map(|n| format!("{base}-{n}")))
        .map(|name| recording_rel(code, &name))
        .find(|rel| !taken(rel))
        .expect("an unbounded range has a free name")
}

/// `<YYYY-MM-DD>-<slug>`: the lecture's local start date, and its title
/// lowercased and folded to ASCII, with the subject code's words dropped
/// (Echo360 titles are mostly the timetable's `MULT20015_2026_SM2 MO L105`).
pub fn recording_name(date: &str, title: &str, code: &str) -> String {
    let code_words = words(code);
    let mut slug = words(title)
        .into_iter()
        .filter(|w| !code_words.contains(w))
        .collect::<Vec<_>>()
        .join("-");
    slug.truncate(SLUG_MAX);
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { UNTITLED } else { slug };
    format!("{}-{slug}", local_day(date))
}

/// Lowercase ASCII words: accents folded away (NFKD, marks dropped), any
/// other character a separator.
fn words(s: &str) -> Vec<String> {
    let folded: String = s.nfkd().filter(char::is_ascii).collect();
    folded
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// The calendar day a lecture started on, where it was given. Echo360's
/// `timing.start` is local wall time with no offset; one with an offset is
/// turned into this machine's local time.
fn local_day(date: &str) -> String {
    if let Ok(at) = chrono::DateTime::parse_from_rfc3339(date) {
        return at
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d")
            .to_string();
    }
    let day: String = date.chars().take(10).collect();
    let shape_ok = day.len() == 10
        && day.char_indices().all(|(i, c)| match i {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        });
    if shape_ok {
        day
    } else {
        "undated".to_string()
    }
}

/// The recording folder a data-dir-relative path sits in (or is), as stored
/// on a lecture's row; `None` for any other path.
pub fn recording_of(rel: &str) -> Option<String> {
    CURRENT.recording_of(rel)
}

/// A recording's folder on disk, from the path its row stores. Refuses
/// anything that is not one recording folder, since a command may be handed
/// one.
pub fn recording_dir(data_dir: &Path, folder_rel: &str) -> Result<PathBuf, String> {
    if recording_of(folder_rel).as_deref() != Some(folder_rel) {
        return Err(format!("{folder_rel:?} is not a recording folder"));
    }
    Ok(data_dir.join(folder_rel))
}

/// Every recording folder on disk, for sweeps over all of them.
pub fn recording_dirs(data_dir: &Path) -> Vec<PathBuf> {
    let roots: Vec<PathBuf> = match CURRENT.recordings {
        Recordings::DataDir(root) => vec![data_dir.join(root)],
        Recordings::Synced(_) => subdirs(&courses_dir(data_dir))
            .into_iter()
            .filter_map(|course| {
                let code = course.file_name()?.to_str()?.to_string();
                Some(data_dir.join(recordings_rel(&code)))
            })
            .collect(),
    };
    roots.iter().flat_map(|root| subdirs(root)).collect()
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

impl Shape {
    /// The data-dir-relative folder holding a subject's recordings.
    pub fn recordings_rel(&self, code: &str) -> String {
        match self.recordings {
            Recordings::DataDir(root) => root.to_string(),
            Recordings::Synced(dir) => in_course(code, &self.under_synced(&[dir])),
        }
    }

    /// The recording folder `rel` is in, spelled as `rel` spells it.
    pub fn recording_of(&self, rel: &str) -> Option<String> {
        if self.zone_of(rel) == Zone::Outside {
            return None;
        }
        let parts = segments(rel)?;
        let depth = match self.recordings {
            Recordings::DataDir(root) => same_name(parts[0], root).then_some(2)?,
            Recordings::Synced(_) => {
                let (code, _) = super::split_course(rel)?;
                let want = self.recordings_rel(code);
                let prefix: Vec<&str> = want.split('/').collect();
                let matches = parts.len() > prefix.len()
                    && parts
                        .iter()
                        .zip(&prefix)
                        .all(|(have, want)| same_name(have, want));
                matches.then_some(prefix.len() + 1)?
            }
        };
        (parts.len() >= depth && !parts[depth - 1].starts_with('.'))
            .then(|| parts[..depth].join("/"))
    }
}

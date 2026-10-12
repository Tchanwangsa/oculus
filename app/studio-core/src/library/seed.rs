//! Seeding: offering the starter set to a library folder and keeping the
//! copies the user never edited current. `.seeded` holds one line per starter
//! file already offered, `path<TAB>sha256 of the text last written`; a line
//! without a tab predates hashes and means "offered, hash unknown".

use super::{shipped::SHIPPED, write};
use crate::error::{Result, StudioError};
use sha2::{Digest, Sha256};
use std::path::Path;

/// The record of starter files already offered to a library.
const SEEDED: &str = ".seeded";

/// `(library path, hashes of every shipped version)`, as `shipped.rs` lists.
pub(super) type Shipped<'a> = &'a [(&'a str, &'a [&'a str])];

/// Seeds the compiled starter set into `root`; see [`super::seed`].
pub(super) fn seed(root: &Path, reset: bool) -> Result<Vec<String>> {
    let files = super::STARTER_COMPONENTS
        .iter()
        .map(|(name, text)| (format!("components/{name}.tsx"), *text))
        .chain(
            super::STARTER_THEMES
                .iter()
                .map(|(name, text)| (format!("themes/{name}.json"), *text)),
        );
    seed_files(root, files, SHIPPED, reset)
}

/// Lower-case hex SHA-256 of `bytes`.
pub(super) fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// One `.seeded` line.
#[derive(Clone, PartialEq)]
struct Offered {
    rel: String,
    /// The hash of the text last written, or `None` for a legacy line.
    hash: Option<String>,
}

fn parse(record: &str) -> Vec<Offered> {
    record
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| match line.split_once('\t') {
            Some((rel, hash)) => Offered {
                rel: rel.to_owned(),
                hash: Some(hash.to_owned()),
            },
            None => Offered {
                rel: line.to_owned(),
                hash: None,
            },
        })
        .collect()
}

fn render(offered: &[Offered]) -> String {
    offered
        .iter()
        .map(|o| match &o.hash {
            Some(h) => format!("{}\t{h}\n", o.rel),
            None => format!("{}\n", o.rel),
        })
        .collect()
}

/// What `seed_files` does with one starter file.
enum Action {
    Leave,
    /// Leave the file, set the record's hash to the starter's.
    Record,
    /// Write the starter text (snapshotting the old), then record its hash.
    Write,
}

pub(super) fn seed_files<'a>(
    root: &Path,
    files: impl Iterator<Item = (String, &'a str)>,
    shipped: Shipped<'_>,
    reset: bool,
) -> Result<Vec<String>> {
    let record = root.join(SEEDED);
    let mut offered = match std::fs::read_to_string(&record) {
        Ok(s) => parse(&s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(StudioError::io(&record, e)),
    };
    let before = offered.clone();
    let mut written = Vec::new();
    for (rel, text) in files {
        let current = hash(text.as_bytes());
        let path = root.join(&rel);
        let on_disk = match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(StudioError::io(&path, e)),
        };
        let at = offered.iter().position(|o| o.rel == rel);
        let known = shipped
            .iter()
            .find(|(p, _)| *p == rel)
            .map_or(&[][..], |(_, hashes)| hashes);
        let recorded = at.and_then(|i| offered[i].hash.as_deref());
        let action = match &on_disk {
            Some(bytes) if reset => {
                if hash(bytes) == current {
                    Action::Record
                } else {
                    Action::Write
                }
            }
            // A file already there with no record is judged like a legacy
            // line: the user's own unless it is a shipped version.
            Some(bytes) => untouched_or_leave(&hash(bytes), &current, recorded, known),
            None if reset || at.is_none() => Action::Write,
            // Offered and deleted: it stays deleted.
            None => Action::Leave,
        };
        let offer = Offered {
            rel: rel.clone(),
            hash: Some(current),
        };
        match action {
            Action::Leave => {
                // Offered once, so a deletion or an edit is respected; the
                // hash stays whatever it was (none for a file that was
                // already there).
                if at.is_none() {
                    offered.push(Offered {
                        hash: None,
                        ..offer
                    });
                }
            }
            Action::Record | Action::Write => {
                if matches!(action, Action::Write) {
                    write(root, &rel, text)?;
                    written.push(rel.clone());
                }
                match at {
                    Some(i) => offered[i] = offer,
                    None => offered.push(offer),
                }
            }
        }
    }
    if offered != before {
        std::fs::create_dir_all(root).map_err(|e| StudioError::io(root, e))?;
        let tmp = record.with_extension("tmp");
        std::fs::write(&tmp, render(&offered)).map_err(|e| StudioError::io(&tmp, e))?;
        std::fs::rename(&tmp, &record).map_err(|e| StudioError::io(&record, e))?;
    }
    Ok(written)
}

/// The action for a file on disk with hash `disk` of a starter whose current
/// hash is `current`, given the recorded hash `recorded` (`None` for a
/// legacy line or a file never offered) and the hashes of every version
/// shipped. The file is untouched when it equals what was last written, or,
/// with no recorded hash, any shipped version. An untouched file is rewritten
/// to the current text; one that already is the current text only gets its
/// record updated; an edited one is left, and its hash is never recorded.
fn untouched_or_leave(
    disk: &str,
    current: &str,
    recorded: Option<&str>,
    shipped: &[&str],
) -> Action {
    if disk == current {
        return if recorded == Some(current) {
            Action::Leave
        } else {
            Action::Record
        };
    }
    let untouched = match recorded {
        Some(r) => disk == r,
        None => shipped.contains(&disk),
    };
    if untouched {
        Action::Write
    } else {
        Action::Leave
    }
}

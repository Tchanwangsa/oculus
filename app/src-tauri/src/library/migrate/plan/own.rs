//! The student's folders (uploads, notes, canvases, notebooks) flattened into
//! the subject's root, keeping everything below them. A file travels with
//! what its parse made of it (its text, pictures, records, converted PDF), so
//! a clash renames them together: `a.pdf` → `a (2).pdf` takes `a (2).md` and
//! `a (2)_images/` with it. A name the layout keeps (`Course`, `.oculus`)
//! becomes `Course (mine)`. The notes' pictures folder takes its hidden name.

use std::collections::HashSet;
use std::path::Path;

use super::walk::record_home;
use super::{base, join, list, parent, suffixed, Builder, Entry};
use crate::library::layout::{self, DerivedKind, FolderMove, FolderMoveKind, LEGACY};
use crate::library::migrate::journal::{Kind, Moved, Op};
use crate::library::paths::{doc_pdf_rel, is_pdf, is_sheet};

/// A `.DS_Store` Finder left; deleted so the folder it sits in can go.
pub(in crate::library::migrate) const FINDER_FILE: &str = ".DS_Store";

pub(super) fn plan(b: &mut Builder, root: &str, moves: &[FolderMove]) {
    let assets = moves.iter().find(|m| m.kind == FolderMoveKind::NoteAssets);
    // The notes folder is the one the pictures folder sits in.
    let notes_dir = assets.map(|m| parent(&m.old).to_string());
    for m in moves.iter().filter(|m| m.kind == FolderMoveKind::Own) {
        let dir = join(root, &m.old);
        if !b.is_dir(&dir) {
            continue;
        }
        let dest = join(root, &m.new);
        b.rules.add(&dir, &dest);
        let is_notes = notes_dir.as_deref() == Some(m.old.as_str());
        let nested = if is_notes {
            nested_assets(b, &dir)
        } else {
            Vec::new()
        };
        flatten(b, root, &dir, &dest, assets.filter(|_| is_notes));
        // A pictures folder below the top was renamed where it stood, before
        // its folder moved: its rule follows the folder.
        for (old, name) in nested {
            let new = join(&b.rules.apply(parent(&old)), &name);
            b.rules.add(&old, &new);
        }
        b.step(Kind::Cleanup, Op::RemoveDirIfEmpty { path: dir });
    }
}

/// Every pictures folder deeper in the notes folder, renamed in place to the
/// hidden name now (steps before the flatten); returned with the name taken.
fn nested_assets(b: &mut Builder, notes: &str) -> Vec<(String, String)> {
    let (old_name, new_name) = layout::note_assets_rename();
    let mut out = Vec::new();
    let mut pending: Vec<String> = list(&b.data_dir.join(notes))
        .into_iter()
        .filter(|(name, kind)| *kind == Entry::Dir && name != old_name)
        .map(|(name, _)| join(notes, &name))
        .collect();
    while let Some(dir) = pending.pop() {
        if b.debris.contains(&dir) {
            continue;
        }
        for (name, kind) in list(&b.data_dir.join(&dir)) {
            if kind != Entry::Dir {
                continue;
            }
            let child = join(&dir, &name);
            if name == old_name {
                let chosen = b.free_name(&dir, new_name, true);
                b.step(
                    Kind::NoteAssets,
                    Op::Rename {
                        from: child.clone(),
                        to: join(&dir, &chosen),
                    },
                );
                out.push((child, chosen));
            } else {
                pending.push(child);
            }
        }
    }
    out
}

/// One own folder's entries into `dest`.
fn flatten(b: &mut Builder, root: &str, dir: &str, dest: &str, assets: Option<&FolderMove>) {
    let entries = list(&b.data_dir.join(dir));
    let present: HashSet<String> = entries.iter().map(|(n, _)| n.clone()).collect();
    let mut claimed: HashSet<String> = HashSet::new();
    for (name, kind) in &entries {
        let child = join(dir, name);
        if b.debris.contains(&child) {
            claimed.insert(name.clone());
        } else if name == FINDER_FILE && *kind == Entry::File {
            claimed.insert(name.clone());
            b.step(Kind::Cleanup, Op::RemoveFile { path: child });
        } else if let Some(a) = assets.filter(|a| base(&a.old) == name) {
            claimed.insert(name.clone());
            let target = join(root, &a.new);
            let chosen = b.free_name(parent(&target), base(&target), true);
            let to = join(parent(&target), &chosen);
            b.rename(Kind::NoteAssets, &child, &to);
        }
    }
    for group in groups(&entries, &present, &claimed) {
        claimed.extend(group.members.iter().cloned());
        place(b, dir, dest, &group);
    }
}

/// A file and what parsing made of it, renamed as one; or a lone entry.
struct Group {
    lead: String,
    lead_is_dir: bool,
    /// The part of every member's name the lead's name sets: the stem for a
    /// PDF (`a` of `a.md`, `a_images`), the whole name for an Office file or
    /// spreadsheet (`x.docx` of `x.docx.pdf`, `x.docx.md`).
    key: String,
    key_is_stem: bool,
    members: Vec<String>,
}

fn groups(
    entries: &[(String, Entry)],
    present: &HashSet<String>,
    claimed: &HashSet<String>,
) -> Vec<Group> {
    // Office files and spreadsheets claim their converted PDF before it can
    // lead a group of its own; then PDFs; then everything else alone.
    let rank = |name: &str| {
        if doc_pdf_rel(name).is_some_and(|p| p != name) || is_sheet(name) {
            0
        } else if is_pdf(name) {
            1
        } else {
            2
        }
    };
    let mut order: Vec<&(String, Entry)> = entries.iter().collect();
    order.sort_by_key(|(name, _)| (rank(name), name.clone()));
    let mut taken: HashSet<String> = claimed.clone();
    let mut out = Vec::new();
    for (name, kind) in order {
        if taken.contains(name) {
            continue;
        }
        let mut members = vec![name.clone()];
        let pdf = (*kind == Entry::File)
            .then(|| doc_pdf_rel(name).or_else(|| is_sheet(name).then(|| format!("{name}.pdf"))))
            .flatten();
        if let Some(pdf) = &pdf {
            let siblings =
                std::iter::once(pdf.clone()).chain(DerivedKind::ALL.iter().map(|k| {
                    crate::library::paths::slash_path(LEGACY.derived(Path::new(pdf), *k))
                }));
            for sibling in siblings {
                if &sibling != name && present.contains(&sibling) && !taken.contains(&sibling) {
                    members.push(sibling);
                }
            }
        }
        let key_is_stem = is_pdf(name);
        let key = if key_is_stem {
            name.rsplit_once('.')
                .map_or(name.as_str(), |(s, _)| s)
                .to_string()
        } else {
            name.clone()
        };
        taken.extend(members.iter().cloned());
        out.push(Group {
            lead: name.clone(),
            lead_is_dir: *kind == Entry::Dir,
            key,
            key_is_stem,
            members,
        });
    }
    out
}

/// A member's name with the lead's renamed to `name (n)`.
fn member_name(group: &Group, member: &str, n: usize) -> String {
    if n < 2 {
        return member.to_string();
    }
    let lead = suffixed(&group.lead, n, group.lead_is_dir);
    if member == group.lead {
        return lead;
    }
    let key = if group.key_is_stem {
        lead.rsplit_once('.')
            .map_or(lead.as_str(), |(s, _)| s)
            .to_string()
    } else {
        lead
    };
    match member.strip_prefix(group.key.as_str()) {
        Some(rest) => format!("{key}{rest}"),
        None => suffixed(member, n, false),
    }
}

fn place(b: &mut Builder, dir: &str, dest: &str, group: &Group) {
    let at_root = dest.matches('/').count() == 1;
    // A lone entry named like a folder the layout keeps steps aside first.
    let reserved = at_root && group.members.len() == 1 && layout::reserved_name(&group.lead);
    let n = (1..)
        .find(|&n| {
            group.members.iter().all(|m| {
                let name = if reserved {
                    suffixed(&layout::reserved_rename(m), n, true)
                } else {
                    member_name(group, m, n)
                };
                !b.taken(dest, &name) && !(at_root && layout::reserved_name(&name))
            })
        })
        .expect("an unbounded range has a free name");
    let pdf_old = group
        .members
        .iter()
        .find(|m| is_pdf(m))
        .cloned()
        .unwrap_or_default();
    let pdf_new = member_name(group, &pdf_old, n);
    for member in &group.members {
        let new_name = if reserved {
            suffixed(&layout::reserved_rename(member), n, true)
        } else {
            member_name(group, member, n)
        };
        let from = join(dir, member);
        let to = join(dest, &new_name);
        b.hold(dest, &new_name);
        if new_name == *member {
            // The folder's own rule already says where it went.
            b.step(
                Kind::Own,
                Op::Rename {
                    from,
                    to: to.clone(),
                },
            );
        } else {
            b.rename(Kind::Own, &from, &to);
            if reserved {
                b.plan.left.push(format!(
                    "{from}: renamed {new_name}, since the layout keeps its name"
                ));
            }
            if let Some(home) = record_home(&to) {
                b.plan.renamed_records.push((
                    home,
                    Moved {
                        old: pdf_old.clone(),
                        new: pdf_new.clone(),
                    },
                ));
            }
        }
    }
}

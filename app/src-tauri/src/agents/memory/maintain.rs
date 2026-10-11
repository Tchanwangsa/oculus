use std::collections::BTreeMap;
use std::path::Path;

use super::bucket::bucket_dir;
use super::frontmatter::{parse, render, Entry, Front};
use super::query::{find, list};
use super::slug::humanize;
use super::write::Written;
use super::{INDEX_MARK, INDEX_NAME};

/// Delete one memory and rewrite the index without it.
pub fn remove(data_dir: &Path, name: &str, code: Option<&str>) -> Result<Entry, String> {
    let entry = find(data_dir, name, code)?;
    std::fs::remove_file(&entry.path).map_err(|e| format!("cannot delete {}: {e}", entry.path))?;
    reindex(data_dir, entry.subject.as_deref())?;
    Ok(entry)
}

/// Move a memory from the bucket it is in to another one.
///
/// Both indexes are rewritten and the file carried across unchanged.
pub fn relocate(data_dir: &Path, entry: &Entry, to: Option<&str>) -> Result<Written, String> {
    if entry.subject.as_deref() == to {
        return Err(format!("{} is already filed there", entry.front.name));
    }
    let dir = bucket_dir(data_dir, to);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let path = dir.join(format!("{}.md", entry.front.name));
    if path.exists() {
        return Err(format!(
            "{} is already filed there under that name",
            entry.front.name
        ));
    }
    std::fs::rename(&entry.path, &path)
        .map_err(|e| format!("cannot move {} to {}: {e}", entry.path, path.display()))?;
    reindex(data_dir, entry.subject.as_deref())?;
    let indexed = reindex(data_dir, to)?;
    Ok(Written {
        name: entry.front.name.clone(),
        path: path.to_string_lossy().into_owned(),
        subject: to.map(String::from),
        created: false,
        indexed,
    })
}

/// Rewrite one bucket's `MEMORY.md` from the files in it.
///
/// The index is derived, so an entry cannot go missing or linger. Prose above
/// the generated block is kept.
pub fn reindex(data_dir: &Path, code: Option<&str>) -> Result<usize, String> {
    let dir = bucket_dir(data_dir, code);
    let path = dir.join(INDEX_NAME);
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let entries = list(data_dir, code)?;

    // Titles already written in the index are adopted, not replaced.
    let inherited = index_titles(&old);

    let mut out = String::new();
    out.push_str(preamble(&old, code).trim_end());
    out.push_str("\n\n");
    out.push_str(INDEX_MARK);
    out.push('\n');
    if entries.is_empty() {
        out.push_str("\nNothing filed here yet.\n");
    } else {
        out.push('\n');
        for e in &entries {
            let file = format!("{}.md", e.front.name);
            let inherited_title = inherited.get(&file).map(String::as_str);
            // Stamp what the file is missing (a hand-written index title, undated
            // creation) into it once, so the index derives from files alone.
            backfill(&e.path, inherited_title);
            let title = match (&e.front.title, inherited_title) {
                (Some(title), _) => title.clone(),
                (None, Some(title)) => title.to_string(),
                (None, None) => humanize(&e.front.name),
            };
            out.push_str(&format!(
                "- [{title}]({file}) — {}\n",
                e.front.description.trim()
            ));
        }
    }
    std::fs::write(&path, out).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(entries.len())
}

/// The part of an index that is not the list: down to the marker, else to the
/// first hand-written entry, else a stub.
fn preamble(old: &str, code: Option<&str>) -> String {
    if let Some(cut) = old.find(INDEX_MARK) {
        return old[..cut].to_string();
    }
    let mut kept = String::new();
    for line in old.lines() {
        let t = line.trim_start();
        if t.starts_with("- [") || t.starts_with("<!-- - [") {
            break;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    if !kept.trim().is_empty() {
        return kept;
    }
    // A bucket written before it was ever stubbed.
    let scope = match code {
        Some(code) => format!(
            "# Memories — {code}\n\nFacts about this subject alone — including ones that \
             came up while working on\nsomething else. Anything true across subjects, or about \
             the student themselves,\ngoes in the library's own `agents/memories/`.\n"
        ),
        None => "# Memories — all subjects\n\nObservations that hold across more than one \
                 subject, and facts about the\nstudent themselves. Anything that names **one** \
                 subject goes in the `<CODE>/`\nfolder beside this file; standing preferences \
                 go in `../TASTE.md`.\n"
            .to_string(),
    };
    format!(
        "{scope}\nThe list below is written by `oculus memory` from the files beside it, so it\n\
         cannot fall behind them. Everything above the marker is yours.\n"
    )
}

/// Stamp into a memory a title from the index and a missing `created` date
/// (never `updated` — see [`Entry::dates`]). Best-effort: an index rewrite is
/// not the place to fail over somebody's file.
fn backfill(path: &str, title: Option<&str>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Some((mut front, body)) = parse(&text) else {
        return;
    };
    let mut touched = false;
    if front.title.is_none() {
        if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
            front.title = Some(title.to_string());
            touched = true;
        }
    }
    let file = Path::new(path);
    if front.meta("created").is_none() {
        if let Some(date) = born(file) {
            front.set_meta("created", &date);
            touched = true;
        }
    }
    if touched {
        order_meta(&mut front);
        let _ = std::fs::write(path, render(&front, &body));
    }
}

/// `type` first, then the dates.
pub(super) fn order_meta(front: &mut Front) {
    front.metadata.sort_by_key(|(k, _)| match k.as_str() {
        "type" => 0,
        "created" => 1,
        "updated" => 2,
        _ => 3,
    });
}

/// `- [Title](file.md) — hook` back to `file.md -> Title`.
fn index_titles(old: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in old.lines() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix("- [") else {
            continue;
        };
        let Some((title, rest)) = rest.split_once("](") else {
            continue;
        };
        let Some((file, _)) = rest.split_once(')') else {
            continue;
        };
        map.insert(file.to_string(), title.to_string());
    }
    map
}

/// When an undated file most plausibly began: its birth time, unless that is
/// after its last write (a copy carries the copy's birth time).
pub(super) fn born(path: &Path) -> Option<String> {
    let modified = stat_date(path, std::fs::Metadata::modified);
    match (
        stat_date(path, std::fs::Metadata::created),
        modified.clone(),
    ) {
        (Some(born), Some(edited)) => Some(born.min(edited)),
        (born, edited) => born.or(edited),
    }
}

fn stat_date(
    path: &Path,
    pick: fn(&std::fs::Metadata) -> std::io::Result<std::time::SystemTime>,
) -> Option<String> {
    let secs = std::fs::metadata(path)
        .and_then(|m| pick(&m))
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some(crate::runtime::clock::ymd(secs / 86_400))
}

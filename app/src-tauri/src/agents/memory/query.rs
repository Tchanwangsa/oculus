use std::path::Path;

use super::bucket::{bucket_dir, buckets};
use super::frontmatter::{parse, Entry};
use super::maintain::born;
use super::slug::slug;
use super::INDEX_NAME;

/// Every memory in one bucket, oldest first.
///
/// By `created` (an undated file falls back to its mtime), so the index reads
/// as the store grew and a rewrite does not shuffle it.
pub fn list(data_dir: &Path, code: Option<&str>) -> Result<Vec<Entry>, String> {
    let dir = bucket_dir(data_dir, code);
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut rows: Vec<(String, Entry)> = Vec::new();
    for e in rd.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("md") {
            continue;
        }
        let file = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if file == INDEX_NAME {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let stem = file.trim_end_matches(".md").to_string();
        let (mut front, _) = parse(&text).unwrap_or_default();
        if front.name.trim().is_empty() {
            front.name = stem;
        }
        // The same fallback [`Entry::dates`] displays.
        let key = front
            .meta("created")
            .map(String::from)
            .unwrap_or_else(|| born(&path).unwrap_or_default());
        rows.push((
            format!("{key}\u{0}{}", front.name),
            Entry {
                front,
                subject: code.map(String::from),
                path: path.to_string_lossy().into_owned(),
                body: None,
            },
        ));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(rows.into_iter().map(|(_, e)| e).collect())
}

/// One memory by name, with its body, from the bucket given or from whichever
///
/// Searched across buckets, since a caller rarely knows which one. The match is
/// a ladder: exact name, then spellings slugged from the title, then prefix,
/// then fragment; a lower rung runs only if the one above found nothing. A rung
/// with several answers lists them rather than picking, so `rm` never takes a
/// neighbour.
pub fn find(data_dir: &Path, name: &str, code: Option<&str>) -> Result<Entry, String> {
    let wanted = slug(name);
    if wanted.is_empty() {
        return Err(format!(
            "{name} is not a name — `oculus memory list --all` has them"
        ));
    }
    let scope: Vec<Option<String>> = if code.is_some() {
        vec![code.map(String::from)]
    } else {
        buckets(data_dir)
    };
    let mut pool: Vec<Entry> = Vec::new();
    for bucket in &scope {
        pool.extend(list(data_dir, bucket.as_deref())?);
    }

    // Below the first rung hyphens are ignored on both sides: where they fall is
    // all a slug from a title cannot know.
    let bare = |s: &str| s.replace('-', "");
    let want = bare(&wanted);
    let rung = |test: &dyn Fn(&str) -> bool| -> Vec<Entry> {
        pool.iter()
            .filter(|e| {
                [&e.front.name, &e.front.display()]
                    .iter()
                    .any(|s| test(&bare(&slug(s))))
            })
            .cloned()
            .collect()
    };

    let mut hits: Vec<Entry> = pool
        .iter()
        .filter(|e| slug(&e.front.name) == wanted)
        .cloned()
        .collect();
    if hits.is_empty() {
        hits = rung(&|s| s == want);
    }
    if hits.is_empty() {
        hits = rung(&|s| s.starts_with(&want));
    }
    if hits.is_empty() {
        hits = rung(&|s| s.contains(&want));
    }

    match hits.len() {
        1 => {
            let mut entry = hits.remove(0);
            let text = std::fs::read_to_string(&entry.path)
                .map_err(|e| format!("cannot read {}: {e}", entry.path))?;
            entry.body = Some(parse(&text).map(|(_, b)| b).unwrap_or(text));
            Ok(entry)
        }
        0 => Err(format!("no memory called {name}{}", nearby(&pool, &wanted))),
        // One name in two buckets needs a different flag than an ambiguous guess.
        _ if hits.iter().all(|h| h.front.name == hits[0].front.name) => Err(format!(
            "{name} is filed in {} buckets ({}) — pass -s",
            hits.len(),
            hits.iter()
                .map(|h| h
                    .subject
                    .clone()
                    .unwrap_or_else(|| "across subjects".into()))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        _ => Err(format!(
            "{name} matches {} memories ({}) — pass one of those names",
            hits.len(),
            hits.iter()
                .map(|h| h.front.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// What the scope does hold, most words shared with the guess first, capped.
fn nearby(pool: &[Entry], wanted: &str) -> String {
    if pool.is_empty() {
        return String::new();
    }
    let words: Vec<&str> = wanted.split('-').filter(|w| w.len() > 2).collect();
    let mut names: Vec<(usize, &str)> = pool
        .iter()
        .map(|e| {
            let name = e.front.name.as_str();
            (words.iter().filter(|w| name.contains(**w)).count(), name)
        })
        .collect();
    names.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    let more = names.len().saturating_sub(8);
    names.truncate(8);
    format!(
        " — the store has {}{}",
        names.iter().map(|(_, n)| *n).collect::<Vec<_>>().join(", "),
        if more > 0 {
            format!(", and {more} more")
        } else {
            String::new()
        }
    )
}

//! The memory store the agents write back into, and the one writer for it.
//!
//! [`crate::agents`] scaffolds the layer and never writes a memory; this module
//! does, on an agent's behalf, through `oculus memory`.
//!
//! A memory is two writes — the file and its line in `MEMORY.md` — and agents
//! skip the second. So the index is **derived from the files** and rewritten on
//! every write, and routing is a flag, not a path to reason out.
//!
//! Nothing here touches the database: a sandboxed in-app thread cannot write
//! `oculus.db`, but may write files under `agents/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::agents;

/// The four kinds a memory can be, the templates' vocabulary. `feedback` and
/// `project` also owe a **Why** and a **How to apply** — see [`WriteSpec`].
pub const TYPES: [&str; 4] = ["user", "feedback", "project", "reference"];

/// The generated half of a `MEMORY.md` starts here; prose above it is kept.
const INDEX_MARK: &str =
    "<!-- Written by `oculus memory` from the files beside this one — edit a memory, not this list. -->";

const INDEX_NAME: &str = "MEMORY.md";

/// Where one bucket lives. `None` is the cross-subject one.
pub fn bucket_dir(data_dir: &Path, code: Option<&str>) -> PathBuf {
    match code {
        Some(code) => agents::subject_memories(data_dir, code),
        None => agents::agents_dir(data_dir).join("memories"),
    }
}

/// A subject code to the course folder its bucket is named for.
///
/// Resolved on the filesystem, not the subjects table, so it works from a
/// sandbox. A bare code matches the folder it prefixes; a bucket without a
/// course folder still resolves, so an unsynced subject keeps its memories.
pub fn resolve_subject(data_dir: &Path, code: &str) -> Result<String, String> {
    let wanted = code.trim().to_ascii_uppercase();
    if wanted.is_empty() {
        return Err("a subject code cannot be empty".into());
    }
    let mut names: Vec<String> = Vec::new();
    for dir in [data_dir.join("courses"), bucket_dir(data_dir, None)] {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let Some(name) = e.file_name().to_str().map(String::from) else {
                continue;
            };
            if name.starts_with('.') || names.contains(&name) {
                continue;
            }
            names.push(name);
        }
    }
    names.sort();
    let hit: Vec<&String> = names
        .iter()
        .filter(|n| n.to_ascii_uppercase().starts_with(&wanted))
        .collect();
    match hit.len() {
        1 => Ok(hit[0].clone()),
        0 => Err(format!(
            "no subject matches {code}{}",
            if names.is_empty() {
                String::new()
            } else {
                format!(" — the library has {}", names.join(", "))
            }
        )),
        _ => Err(format!(
            "{code} matched {} subjects ({}) — pass the full folder name",
            hit.len(),
            hit.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// A filename for a fact, from whatever the caller had to hand.
///
/// Lossy on purpose: a whole sentence gets a usable slug, not a refusal.
pub fn slug(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    // Readable in a listing, cut back to a whole word.
    match out.char_indices().nth(64) {
        None => out,
        Some((cut, _)) => {
            let head = &out[..cut];
            head.rsplit_once('-')
                .map(|(k, _)| k)
                .unwrap_or(head)
                .to_string()
        }
    }
}

/// A filename from the description's first clause, at most six words.
fn name_from(about: &str) -> String {
    let clause = about
        .split(['.', ';', ':', '—', ','])
        .next()
        .unwrap_or(about);
    let words: Vec<&str> = clause.split_whitespace().take(6).collect();
    slug(&words.join(" "))
}

/// A slug back to words, for an index line whose file has no title.
pub fn humanize(name: &str) -> String {
    let mut s = name.replace(['-', '_'], " ");
    if let Some(first) = s.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    s
}

/// The front matter of one memory.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Front {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub description: String,
    /// Ordered, so `type` stays on top and an unknown key survives a rewrite.
    /// Serialized as an object so `--json` offers `metadata.type`.
    #[serde(serialize_with = "as_map")]
    pub metadata: Vec<(String, String)>,
}

fn as_map<S: serde::Serializer>(
    pairs: &[(String, String)],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_map(pairs.iter().map(|(k, v)| (k, v)))
}

impl Front {
    pub fn meta(&self, key: &str) -> Option<&str> {
        self.metadata
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn set_meta(&mut self, key: &str, value: &str) {
        match self.metadata.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value.to_string(),
            None => self.metadata.push((key.to_string(), value.to_string())),
        }
    }

    /// The file's title, else the slug read back as words.
    pub fn display(&self) -> String {
        self.title.clone().unwrap_or_else(|| humanize(&self.name))
    }
}

/// One memory on disk.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Entry {
    #[serde(flatten)]
    pub front: Front,
    /// The course folder this is filed under, or `None` for cross-subject.
    pub subject: Option<String>,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl Entry {
    /// When the memory was created and when it was last revised.
    ///
    /// `created` falls back to the filesystem, as the listing order does;
    /// [`reindex`] stamps it into the file once. `updated` has no fallback: an
    /// mtime would date a fact by its housekeeping.
    pub fn dates(&self) -> (Option<String>, Option<String>) {
        (
            self.front
                .meta("created")
                .map(String::from)
                .or_else(|| born(Path::new(&self.path))),
            self.front.meta("updated").map(String::from),
        )
    }
}

/// Split a memory file into its front matter and everything after it.
///
/// Hand-rolled rather than a YAML dependency: the shape is fixed and shallow,
/// and hand-written files must still load. Unparseable is `None`; nothing here
/// rewrites a file it did not understand.
pub fn parse(text: &str) -> Option<(Front, String)> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let (head, tail) = rest.split_at(end);
    let body = tail
        .trim_start_matches("\n---")
        .trim_start_matches('\n')
        .to_string();

    let mut front = Front::default();
    let mut in_meta = false;
    for line in head.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = unquote(value.trim());
        if indented && in_meta {
            front.metadata.push((key.to_string(), value));
            continue;
        }
        in_meta = false;
        match key {
            "name" => front.name = value,
            "title" => front.title = Some(value).filter(|v| !v.is_empty()),
            "description" => front.description = value,
            "metadata" => in_meta = true,
            _ => {}
        }
    }
    Some((front, body))
}

fn unquote(value: &str) -> String {
    let v = value.trim();
    for q in ['"', '\''] {
        if v.len() >= 2 && v.starts_with(q) && v.ends_with(q) {
            return v[1..v.len() - 1].replace(&format!("\\{q}"), &q.to_string());
        }
    }
    v.to_string()
}

/// A scalar that reads back as itself; quoted only where YAML would see structure.
fn scalar(value: &str) -> String {
    let v = value.replace(['\n', '\r'], " ");
    let needs = v.is_empty()
        || v.contains(": ")
        || v.ends_with(':')
        || v.contains(" #")
        || v.starts_with([
            '"', '\'', '[', '{', '*', '&', '!', '|', '>', '%', '@', '`', '-', '?',
        ])
        || v.trim() != v;
    if needs {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        v
    }
}

/// The file, front matter and all.
pub fn render(front: &Front, body: &str) -> String {
    let mut s = String::from("---\n");
    s.push_str(&format!("name: {}\n", scalar(&front.name)));
    if let Some(title) = &front.title {
        s.push_str(&format!("title: {}\n", scalar(title)));
    }
    s.push_str(&format!("description: {}\n", scalar(&front.description)));
    if !front.metadata.is_empty() {
        s.push_str("metadata:\n");
        for (k, v) in &front.metadata {
            s.push_str(&format!("  {k}: {}\n", scalar(v)));
        }
    }
    s.push_str("---\n\n");
    s.push_str(body.trim_end());
    s.push('\n');
    s
}

/// What `oculus memory write` was asked for. Absent fields are kept from the
/// existing file or derived.
#[derive(Default)]
pub struct WriteSpec {
    pub name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub body: Option<String>,
    pub why: Option<String>,
    pub how: Option<String>,
    pub source: Option<String>,
    pub links: Vec<String>,
    pub extra: Vec<(String, String)>,
}

/// What [`write`] did, for human and `--json` output.
#[derive(Debug, serde::Serialize)]
pub struct Written {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// False when this replaced an existing memory.
    pub created: bool,
    /// How many entries the index has now.
    pub indexed: usize,
}

/// Write one memory, and rewrite the index it belongs to.
///
/// An upsert: an existing name is updated in place, keeping its `created` and
/// anything the caller did not pass — the templates' "update the file that
/// already covers it".
pub fn write(data_dir: &Path, code: Option<&str>, spec: WriteSpec) -> Result<Written, String> {
    let dir = bucket_dir(data_dir, code);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    let name = match spec.name.as_deref().map(slug).filter(|s| !s.is_empty()) {
        Some(name) => name,
        // A missing name is derived from the one-liner rather than refused.
        None => {
            let derived = name_from(spec.description.as_deref().unwrap_or_default());
            if derived.is_empty() {
                return Err("pass a name, or an --about line to take one from".into());
            }
            derived
        }
    };

    let path = dir.join(format!("{name}.md"));
    let existing = std::fs::read_to_string(&path).ok().and_then(|t| parse(&t));
    let created = existing.is_none();

    let (mut front, old_body) = existing.unwrap_or_default();
    front.name = name.clone();
    if let Some(title) = spec.title {
        front.title = Some(title);
    }
    if let Some(about) = spec.description {
        front.description = about;
    }
    if front.description.trim().is_empty() {
        return Err("--about is the line the index shows; a memory needs one".into());
    }

    let kind = match spec.kind {
        Some(k) => k,
        None => front
            .meta("type")
            .map(String::from)
            .ok_or("--type is one of user, feedback, project, reference")?,
    };
    if !TYPES.contains(&kind.as_str()) {
        return Err(format!("unknown type {kind} — one of {}", TYPES.join(", ")));
    }

    let mut body = spec.body.unwrap_or(old_body).trim().to_string();
    if let Some(why) = spec.why.as_deref().filter(|w| !w.trim().is_empty()) {
        if !body.contains("**Why:**") {
            body.push_str(&format!("\n\n**Why:** {}", why.trim()));
        }
    }
    if let Some(how) = spec.how.as_deref().filter(|h| !h.trim().is_empty()) {
        if !body.contains("**How to apply:**") {
            body.push_str(&format!("\n\n**How to apply:** {}", how.trim()));
        }
    }
    if !spec.links.is_empty() {
        let links: Vec<String> = spec
            .links
            .iter()
            .map(|l| format!("[[{}]]", slug(l)))
            .filter(|l| !body.contains(l.as_str()))
            .collect();
        if !links.is_empty() {
            body.push_str(&format!("\n\nSee also {}.", links.join(", ")));
        }
    }
    if body.trim().is_empty() {
        return Err("a memory with no body is an index line — pass --text or --body".into());
    }
    // Instructions rather than observations owe the reason and the application,
    // or a later session cannot act on them.
    if matches!(kind.as_str(), "feedback" | "project")
        && !(body.contains("**Why:**") && body.contains("**How to apply:**"))
    {
        return Err(format!(
            "a {kind} memory carries **Why:** and **How to apply:** — pass --why and --how"
        ));
    }

    // UTC, the clock `datetime('now')` reads, so memory and task dates line up.
    let today = crate::clock::today_utc();
    if front.meta("created").is_none() {
        front.set_meta("created", &today);
    }
    front.set_meta("type", &kind);
    front.set_meta("updated", &today);
    if let Some(source) = spec.source {
        front.set_meta("source", &source);
    }
    for (k, v) in spec.extra {
        front.set_meta(&k, &v);
    }
    order_meta(&mut front);

    std::fs::write(&path, render(&front, &body))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let indexed = reindex(data_dir, code)?;

    Ok(Written {
        name,
        path: path.to_string_lossy().into_owned(),
        subject: code.map(String::from),
        created,
        indexed,
    })
}

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

/// Every bucket that exists, the cross-subject one first.
pub fn buckets(data_dir: &Path) -> Vec<Option<String>> {
    let mut out = vec![None];
    if let Ok(rd) = std::fs::read_dir(bucket_dir(data_dir, None)) {
        let mut codes: Vec<String> = rd
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .filter(|n| !n.starts_with('.'))
            .collect();
        codes.sort();
        out.extend(codes.into_iter().map(Some));
    }
    out
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
fn order_meta(front: &mut Front) {
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
fn born(path: &Path) -> Option<String> {
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
    Some(crate::clock::ymd(secs / 86_400))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    fn scratch(name: &str) -> Scratch {
        let root = Scratch::new(&format!("memory-{name}"));
        std::fs::create_dir_all(root.join("courses/INFO30006_2026_SM2")).unwrap();
        root
    }

    fn spec(about: &str, body: &str, kind: &str) -> WriteSpec {
        WriteSpec {
            description: Some(about.into()),
            body: Some(body.into()),
            kind: Some(kind.into()),
            ..Default::default()
        }
    }

    fn index(root: &Path, code: Option<&str>) -> String {
        std::fs::read_to_string(bucket_dir(root, code).join(INDEX_NAME)).unwrap()
    }

    /// The file and its index line are one call.
    #[test]
    fn a_memory_and_its_index_are_written_together() {
        let root = scratch("one-write");
        let out = write(
            &root,
            None,
            spec("Wants the verdict first", "Asks for one.", "user"),
        )
        .unwrap();

        let file = std::fs::read_to_string(&out.path).unwrap();
        assert!(
            file.starts_with("---\n"),
            "front matter is written for the caller"
        );
        assert!(file.contains("type: user"));
        assert!(
            file.contains("created: "),
            "and the dates are not the agent's to remember"
        );
        assert!(file.contains("updated: "));

        let index = index(&root, None);
        assert!(
            index.contains("](wants-the-verdict-first.md)"),
            "the index has its line"
        );
        assert!(
            index.contains("Wants the verdict first"),
            "and the hook is the description"
        );
        assert_eq!(out.indexed, 1);
    }

    /// Derived, not appended, so a deletion is complete.
    #[test]
    fn the_index_is_derived_from_the_files_beside_it() {
        let root = scratch("derived");
        write(&root, None, spec("First fact", "One.", "reference")).unwrap();
        write(&root, None, spec("Second fact", "Two.", "reference")).unwrap();
        assert_eq!(index(&root, None).matches("- [").count(), 2);

        remove(&root, "first-fact", None).unwrap();
        let after = index(&root, None);
        assert_eq!(after.matches("- [").count(), 1);
        assert!(
            !after.contains("first-fact"),
            "a deleted memory leaves no line behind"
        );

        // A file dropped in by hand is picked up by the next rewrite.
        std::fs::write(
            bucket_dir(&root, None).join("by-hand.md"),
            "---\nname: by-hand\ndescription: Written without the command\nmetadata:\n  type: user\n---\n\nStill a memory.\n",
        )
        .unwrap();
        assert_eq!(reindex(&root, None).unwrap(), 2);
        assert!(index(&root, None).contains("](by-hand.md)"));
    }

    /// An update keeps what the caller leaves out.
    #[test]
    fn a_name_already_filed_is_updated_and_keeps_what_was_not_passed() {
        let root = scratch("upsert");
        let first = write(&root, None, spec("A fact", "The body.", "reference")).unwrap();
        assert!(first.created);

        // Passing the name says "this one"; a derived name from a new line is a new memory.
        let second = write(
            &root,
            None,
            WriteSpec {
                name: Some("a-fact".into()),
                description: Some("A better line".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!second.created, "the same name is one memory, not two");
        assert_eq!(second.indexed, 1);

        let file = std::fs::read_to_string(&second.path).unwrap();
        assert!(file.contains("A better line"), "the new line landed");
        assert!(
            file.contains("The body."),
            "the body it did not pass survived"
        );
        assert!(file.contains("type: reference"), "and so did the type");
    }

    /// Instruction types must carry a reason and an application.
    #[test]
    fn an_instruction_memory_without_its_why_is_refused() {
        let root = scratch("why");
        let err = write(&root, None, spec("Prefers bun", "Uses bun.", "feedback")).unwrap_err();
        assert!(err.contains("--why"), "{err}");

        let ok = write(
            &root,
            None,
            WriteSpec {
                why: Some("npm writes a second lockfile.".into()),
                how: Some("Reach for bun and bunx.".into()),
                ..spec("Prefers bun", "Uses bun.", "feedback")
            },
        )
        .unwrap();
        let file = std::fs::read_to_string(&ok.path).unwrap();
        assert!(file.contains("**Why:** npm writes"));
        assert!(file.contains("**How to apply:** Reach for bun"));
    }

    /// A subject code resolves on disk, not in the database.
    #[test]
    fn a_subject_fact_is_filed_under_the_subject() {
        let root = scratch("buckets");
        let code = resolve_subject(&root, "INFO30006").unwrap();
        assert_eq!(
            code, "INFO30006_2026_SM2",
            "a bare code matches the folder it prefixes"
        );

        write(
            &root,
            Some(&code),
            spec("The MST is on Friday", "Week 7.", "project"),
        )
        .unwrap_err();
        write(
            &root,
            Some(&code),
            WriteSpec {
                why: Some("It gates the week's plan.".into()),
                how: Some("Check it before planning past Friday.".into()),
                ..spec("The MST is on Friday", "Week 7.", "project")
            },
        )
        .unwrap();

        assert!(index(&root, Some(&code)).contains("The MST is on Friday"));
        assert_eq!(
            list(&root, None).unwrap().len(),
            0,
            "and not in the cross-subject bucket"
        );
        assert!(
            resolve_subject(&root, "COMP90007").is_err(),
            "an unknown code is not guessed at"
        );
    }

    /// Moving a misfiled memory rewrites both indexes.
    #[test]
    fn moving_a_memory_rewrites_both_indexes() {
        let root = scratch("move");
        let code = resolve_subject(&root, "INFO30006").unwrap();
        write(
            &root,
            None,
            spec("An INFO30006 fact", "Filed wrong.", "reference"),
        )
        .unwrap();

        let entry = find(&root, "an-info30006-fact", None).unwrap();
        let moved = relocate(&root, &entry, Some(&code)).unwrap();
        assert_eq!(moved.subject.as_deref(), Some(code.as_str()));

        assert!(
            !index(&root, None).contains("an-info30006-fact"),
            "gone from the old index"
        );
        assert!(
            index(&root, Some(&code)).contains("an-info30006-fact"),
            "and in the new one"
        );
        assert!(
            find(&root, "an-info30006-fact", None).is_ok(),
            "still findable across buckets"
        );
    }

    /// Hand-written indexes and prose are kept, and their titles adopted into files.
    #[test]
    fn a_hand_written_index_keeps_its_prose_and_its_titles() {
        let root = scratch("legacy");
        let dir = bucket_dir(&root, None);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("tchan-study-workflow.md"),
            "---\nname: tchan-study-workflow\ndescription: ROI-first triage; wants a verdict.\nmetadata:\n  type: user\n---\n\nTriages by return on investment.\n",
        )
        .unwrap();
        std::fs::write(
            dir.join(INDEX_NAME),
            "# Memories — all subjects\n\nStanding preferences live in `../TASTE.md`.\n\n- [Tanat's study workflow](tchan-study-workflow.md) — ROI-first triage.\n",
        )
        .unwrap();

        reindex(&root, None).unwrap();
        let after = index(&root, None);
        assert!(
            after.contains("Standing preferences live in"),
            "their prose is kept"
        );
        assert!(
            after.contains("[Tanat's study workflow]"),
            "and so is the title they wrote"
        );

        let file = std::fs::read_to_string(dir.join("tchan-study-workflow.md")).unwrap();
        assert!(
            file.contains("title: Tanat's study workflow"),
            "adopted into the file: {file}"
        );

        // And its dates, stamped once from the filesystem.
        assert!(
            file.contains("created: 20"),
            "dated from the filesystem: {file}"
        );
        assert!(
            !file.contains("updated:"),
            "but not an `updated` — an mtime moves when this rewrite touches the file: {file}"
        );
        let entry = find(&root, "tchan-study-workflow", None).unwrap();
        assert_eq!(entry.dates().0.as_deref(), entry.front.meta("created"));
        assert_eq!(
            entry.dates().1,
            None,
            "nobody has revised it through the command"
        );
    }

    /// A name is derived, and stays readable in a listing.
    #[test]
    fn a_write_that_names_nothing_takes_a_name_from_its_line() {
        let root = scratch("naming");
        let out = write(
            &root,
            None,
            spec(
                "Ed answers are the marking authority for INFO30006; the brief is not",
                "Staff said so in Ed #66.",
                "reference",
            ),
        )
        .unwrap();
        assert_eq!(out.name, "ed-answers-are-the-marking-authority");

        assert_eq!(
            slug("INFO30006 — Week 3 (slides)"),
            "info30006-week-3-slides"
        );
        assert!(
            !slug(&"a-very-long-name-".repeat(9)).ends_with('-'),
            "a cut never lands mid-word or on a dash"
        );
        assert!(slug(&"word ".repeat(40)).len() <= 64);
    }

    /// A guess slugged from the title lands, and a miss lists what is there.
    #[test]
    fn a_name_read_off_the_title_still_finds_the_file() {
        let root = scratch("find");
        write(
            &root,
            None,
            WriteSpec {
                name: Some("info30006-topic4-group-report-progress".into()),
                title: Some("INFO30006 Topic 4 group-report progress".into()),
                why: Some("It is the week's work.".into()),
                how: Some("Read it before planning the report.".into()),
                ..spec(
                    "Group 15's report on the Great Firewall",
                    "The draft.",
                    "project",
                )
            },
        )
        .unwrap();
        write(
            &root,
            None,
            spec("Ed is the marking authority", "Staff said so.", "reference"),
        )
        .unwrap();

        let name = |q: &str| find(&root, q, None).map(|e| e.front.name);
        assert_eq!(
            name("info30006-topic4-group-report-progress").unwrap(),
            "info30006-topic4-group-report-progress"
        );
        assert_eq!(
            name("info30006-topic-4-group-report-progress").unwrap(),
            "info30006-topic4-group-report-progress",
            "the hyphen a title slugs to is not where the file puts it"
        );
        assert!(
            name("INFO30006 Topic 4 group-report progress").is_ok(),
            "the title itself works"
        );
        assert!(
            name("info30006-topic4").is_ok(),
            "and a prefix, while it is unambiguous"
        );
        assert!(
            name("group-report").is_ok(),
            "and a fragment, from the middle of one"
        );
        // The description is not searched: a sentence matches too much for `rm`.
        assert!(
            name("great-firewall").is_err(),
            "a fragment of the description is not a name"
        );

        // A guess that fits two memories must not pick one.
        write(
            &root,
            None,
            spec("Ed is where the deadlines land", "Also Ed.", "reference"),
        )
        .unwrap();
        let both = find(&root, "ed-is", None).unwrap_err();
        assert!(
            both.contains("ed-is-the-marking-authority"),
            "both names are named: {both}"
        );
        assert!(
            both.contains("ed-is-where-the-deadlines"),
            "both names are named: {both}"
        );

        let miss = find(&root, "something-else-entirely", None).unwrap_err();
        assert!(miss.starts_with("no memory called"), "{miss}");
        assert!(
            miss.contains("the store has"),
            "a miss says what is there: {miss}"
        );
    }

    /// Front matter round-trips, including an unknown key.
    #[test]
    fn front_matter_round_trips_including_what_this_build_does_not_know() {
        let text = "---\nname: a-fact\ntitle: A fact\ndescription: \"One line: with a colon\"\nmetadata:\n  type: user\n  provenance: thread 26\n---\n\nThe body.\n";
        let (front, body) = parse(text).unwrap();
        assert_eq!(front.name, "a-fact");
        assert_eq!(front.title.as_deref(), Some("A fact"));
        assert_eq!(front.description, "One line: with a colon");
        assert_eq!(front.meta("provenance"), Some("thread 26"));
        assert_eq!(body.trim(), "The body.");

        let again = render(&front, &body);
        let (front2, body2) = parse(&again).unwrap();
        assert_eq!(
            front2.description, front.description,
            "the colon survived the quotes"
        );
        assert_eq!(front2.meta("provenance"), Some("thread 26"));
        assert_eq!(body2.trim(), body.trim());
    }
}

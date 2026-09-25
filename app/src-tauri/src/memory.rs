//! The memory store the agents write back into, and the one writer for it.
//!
//! [`crate::agents`] scaffolds the layer — the two buckets, the `MEMORY.md`
//! index stubbed in each — and deliberately never writes a memory. This
//! module does, on an agent's behalf, through `oculus memory`. The split is
//! the same one the rest of the library keeps: the app owns the shape of the
//! folder, the CLI is the door anything writes through.
//!
//! **Why a command at all, when a memory is a markdown file any agent can
//! write with its own tools.** Because in practice they did not. A memory is
//! two writes — the file, and a line in that folder's `MEMORY.md` — and the
//! second is the one that gets skipped, which is exactly the one that decides
//! whether the next conversation ever opens the first. Measured over a
//! semester of real use, the store held nine files against fifty-one threads,
//! two subjects had an index with nothing in it, and a plainly subject-scoped
//! fact sat in the cross-subject bucket. So the index stopped being something
//! an agent is asked to remember: it is **derived from the files beside it**
//! and rewritten on every write, and the routing is a flag rather than a path
//! the agent has to reason its way to.
//!
//! **Nothing here touches the database**, which is not an accident either. An
//! in-app thread runs sandboxed and cannot write `oculus.db` — a write there
//! comes back as "readonly database" — so a memory store that lived in SQLite
//! would be one the agent that needs it most could not use. Files under
//! `agents/` are the one thing that sandbox allows.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::agents;

/// The four kinds a memory can be, and the same vocabulary the templates
/// teach. `feedback` and `project` additionally owe a **Why** and a **How to
/// apply** — see [`WriteSpec`].
pub const TYPES: [&str; 4] = ["user", "feedback", "project", "reference"];

/// The generated half of a `MEMORY.md` starts here. Everything above it is
/// whoever's prose it was — the stub's, or the student's — and is copied
/// through untouched.
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
/// Resolved against the **filesystem**, not the subjects table, for the same
/// reason nothing else here opens the database: this has to work from a
/// sandbox, and on a machine where the app has never run. A bare code
/// (`INFO30006`) matches the folder it prefixes; the full folder name matches
/// itself. A bucket that exists without a course folder still resolves, so a
/// subject that has been unsynced does not strand what was learned about it.
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
/// Lossy on purpose: an agent that passes a whole sentence gets a usable slug
/// rather than a refusal, which is the difference between a memory written and
/// a memory abandoned halfway.
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
    // Long enough to stay readable in a directory listing, short enough that
    // a sentence passed by mistake does not become the filename entire. Cut
    // back to a word: a name ending mid-word reads as a truncated file rather
    // than as a short one.
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

/// A filename from the one-line description, for a write that did not name
/// itself. The first clause, and at most six words of it — enough to be
/// recognisable in a listing, short of being the sentence again.
fn name_from(about: &str) -> String {
    let clause = about
        .split(['.', ';', ':', '—', ','])
        .next()
        .unwrap_or(about);
    let words: Vec<&str> = clause.split_whitespace().take(6).collect();
    slug(&words.join(" "))
}

/// A slug back to something with spaces in it, for an index line whose file
/// never said what it would like to be called.
pub fn humanize(name: &str) -> String {
    let mut s = name.replace(['-', '_'], " ");
    if let Some(first) = s.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    s
}

/// The front matter of one memory: the two fields every file has, an optional
/// display title, and the `metadata:` block, kept as an ordered list so a key
/// this build does not know about survives a rewrite.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Front {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub description: String,
    /// Ordered, so the file keeps `type` above the bookkeeping — and a key
    /// this build does not know about survives a rewrite in place. Serialized
    /// as an object, because a consumer of `--json` wants
    /// `metadata.type`, not a list of pairs to walk.
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

    /// What the index calls it: what the file asked to be called, else the
    /// slug read back as words.
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
    /// The two dates a reader judges a memory by — how long this has been
    /// sitting here, and when somebody last said it was still true.
    ///
    /// `created` falls back to the filesystem, because every memory written
    /// before `oculus memory` existed is undated and the same fallback
    /// already decides where such a file lands in a listing — so what the
    /// listing *shows* is what put it there. [`reindex`] stamps it into the
    /// file, once.
    ///
    /// **`updated` has no fallback**, and that is the point. An mtime moves
    /// when this module rewrites front matter, when a file is copied, when
    /// anything at all touches the file; reading one as "revised" would date
    /// a fact by its housekeeping. A memory nobody has revisited through the
    /// command says so by having nothing here.
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
/// A hand-rolled reader rather than a YAML dependency, because the shape is
/// fixed and shallow — three scalars and a one-level `metadata:` block — and
/// because a file an agent wrote by hand before this command existed still has
/// to load. Anything unparseable is reported as `None` and the caller decides;
/// nothing here rewrites a file it did not understand.
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

/// A scalar that will read back as itself. Only the shapes YAML would
/// otherwise take for structure need the quotes.
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

/// What `oculus memory write` was asked for. Every field but the body and the
/// type is optional, and the ones that are absent are either kept from the
/// file already there or derived — which is the point of the command: the
/// agent supplies the fact and the flag that files it, and the front matter,
/// the dates and the index are somebody else's job.
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

/// What [`write`] did, in the shape both the human and the `--json` output
/// want.
#[derive(Debug, serde::Serialize)]
pub struct Written {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// False when this replaced a memory that was already there — the case
    /// the templates ask for over writing a second file about one fact.
    pub created: bool,
    /// How many entries the index has now.
    pub indexed: usize,
}

/// Write one memory, and rewrite the index it belongs to.
///
/// An **upsert**: a name that is already filed is updated in place, keeping
/// its `created` date and anything the caller did not pass. That is the
/// behaviour the templates ask for in prose ("update the file that already
/// covers it rather than adding a second") and the one an agent is least
/// likely to perform unprompted, since it would first have to look.
pub fn write(data_dir: &Path, code: Option<&str>, spec: WriteSpec) -> Result<Written, String> {
    let dir = bucket_dir(data_dir, code);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    let name = match spec.name.as_deref().map(slug).filter(|s| !s.is_empty()) {
        Some(name) => name,
        // Naming a fact is the part an agent is worst at and the part that
        // matters least, so a missing name is derived from the one-liner
        // rather than refused.
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
    // The two types that are instructions rather than observations owe the
    // reader the reason and the application, because a bare "he prefers X" is
    // not something a later session can act on or argue with. The flags exist
    // so this is a fill-in rather than a refusal.
    if matches!(kind.as_str(), "feedback" | "project")
        && !(body.contains("**Why:**") && body.contains("**How to apply:**"))
    {
        return Err(format!(
            "a {kind} memory carries **Why:** and **How to apply:** — pass --why and --how"
        ));
    }

    let today = today();
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
/// Ordered by the `created` date rather than by name, so the index reads as
/// the store grew and a rewrite does not shuffle what was already there. A
/// file from before this command existed has no date and falls back to its
/// mtime.
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
        // The same fallback [`Entry::dates`] displays, so the date on a line
        // is the one that put the line where it is.
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
/// one holds it.
///
/// The search across buckets is what makes a half-remembered name usable: an
/// agent that knows a fact exists rarely knows which of the two buckets it
/// ended up in, and a wrong guess would otherwise read as an absence.
///
/// **The spelling is half-remembered too, and this store is what makes it
/// so.** A listing shows the title before anything else, and a caller who
/// slugs the title back lands a hyphen away from the filename — typing
/// `info30006-topic-4-group-report-progress` for a file called
/// `info30006-topic4-group-report-progress`, which is a correct reading of
/// what it was shown. So the match is a ladder: the exact name, then the
/// spellings a caller who only saw the title could have written, then a
/// prefix, then a fragment. A rung is tried only when the one above it found
/// nothing, and a rung that finds several answers with their names instead of
/// picking one — the failure worth avoiding is an `rm` that takes a
/// neighbour, not a `read` that has to be typed twice.
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

    // Below the first rung the hyphens come out of both sides: where they
    // fall is the one thing a caller reading a title cannot know, and it is
    // the only difference a slug still carries once the case and the
    // punctuation are gone.
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
        // One name in two buckets is a different question from two names
        // answering to one guess, and takes a different flag to settle.
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

/// What the scope does hold, for an error that would otherwise send the
/// caller back for a listing it has already paid for. Ordered by the words
/// the guess and the name share, so the one that was meant is at the front,
/// and capped, because this is a line in a terminal rather than the store.
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
/// Both indexes are rewritten, which is the half a delete-and-retype would
/// get wrong, and the file itself is carried across unchanged — a fact that
/// was filed in the wrong place is still the same fact.
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
/// **The index is derived, which is the whole reason this module exists.** An
/// entry cannot go missing from it, a deleted memory cannot linger in it, and
/// neither outcome depends on an agent remembering a second write. Whatever
/// prose sits above the generated block is kept — that half is the stub's, or
/// the student's.
pub fn reindex(data_dir: &Path, code: Option<&str>) -> Result<usize, String> {
    let dir = bucket_dir(data_dir, code);
    let path = dir.join(INDEX_NAME);
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let entries = list(data_dir, code)?;

    // Titles an earlier hand wrote are adopted rather than replaced: the first
    // run of this must not rename everything already filed.
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
            // What the file is missing is stamped back into it, once: a title
            // somebody wrote by hand in this index, and the dates a memory
            // written before this command existed never had. After this the
            // index is derived from the files alone, which is the property
            // the whole module is for — and a memory keeps its title and its
            // age when it moves buckets.
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

/// The part of an index that is not the list: down to the marker if there is
/// one, else down to the first entry a previous hand wrote, else a stub.
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
    // Only reached for a bucket that was written to before `oculus docs` ever
    // stubbed it. Says the same thing those stubs do.
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

/// Write a title into a file that does not have one. Best-effort: a memory
/// that cannot be parsed or written keeps whatever the index says about it,
/// because an index rewrite is not the place to fail over somebody's file.
/// Stamp back into a memory what its file does not carry: a title an earlier
/// hand wrote in the index, and the `created` date a file written before this
/// command existed never had. Only `created` — see [`Entry::dates`] for why
/// an mtime is not an `updated`.
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

/// `type` first, then the dates: the one a reader cares about should not sit
/// below the bookkeeping.
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

/// Today, UTC, as `YYYY-MM-DD`.
///
/// The same clock the database's `datetime('now')` reads, so a memory's date
/// and a task's line up. Day granularity is all a memory has any use for.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil(secs / 86_400)
}

/// When an undated file most plausibly began: its birth time, except where
/// that is later than its last write — a copied or restored file carries a
/// birth time from the copy, and a memory cannot have been written after it
/// was last edited.
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
    Some(civil(secs / 86_400))
}

/// Days since the epoch to a civil date — Howard Hinnant's `civil_from_days`,
/// which is the whole reason this file needs no date crate.
fn civil(days: i64) -> String {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("oculus-memory-{name}"));
        let _ = std::fs::remove_dir_all(&root);
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

    /// The whole point of the command: the file and its index line are one
    /// call, so the half that was always skipped cannot be.
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

    /// Derived, not appended — which is what makes a deletion complete and an
    /// index impossible to leave stale.
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

        // A file dropped in by hand — or by an agent that wrote markdown the
        // old way — is picked up by the next rewrite without being announced.
        std::fs::write(
            bucket_dir(&root, None).join("by-hand.md"),
            "---\nname: by-hand\ndescription: Written without the command\nmetadata:\n  type: user\n---\n\nStill a memory.\n",
        )
        .unwrap();
        assert_eq!(reindex(&root, None).unwrap(), 2);
        assert!(index(&root, None).contains("](by-hand.md)"));
    }

    /// "Update the file that already covers it rather than adding a second" is
    /// the templates' rule and the one an agent would have to look first to
    /// follow. Here it is the default, and what the caller leaves out is kept.
    #[test]
    fn a_name_already_filed_is_updated_and_keeps_what_was_not_passed() {
        let root = scratch("upsert");
        let first = write(&root, None, spec("A fact", "The body.", "reference")).unwrap();
        assert!(first.created);

        // Naming it is what says "this one", so an update passes the name; a
        // write that only derives one from a changed line is a new memory,
        // which is the same rule any file has.
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

    /// The two types that are instructions rather than observations owe a
    /// reason and an application, because a later session can act on neither
    /// without them. The flags make it a fill-in; the check makes it happen.
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

    /// A subject code files a fact where a scoped thread will look for it, and
    /// the code is resolved on disk rather than in the database — this has to
    /// work from a sandbox that cannot open one.
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

    /// A misfiling is the mistake the two buckets exist to make visible, so
    /// correcting one is a command rather than a retype — and both indexes
    /// have to follow the file.
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

    /// The store as it stands today: indexes somebody wrote by hand, above
    /// prose that is theirs. The rewrite keeps both — and adopts the titles
    /// into the files, so the next one does not have to find them here.
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

        // And its dates, which a file written before this command had no way
        // to carry: the filesystem is the only witness left, and stamping it
        // once means the answer stops depending on a copy's mtime.
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

    /// Naming a fact is the part an agent is worst at and the part that
    /// matters least, so it is derived rather than demanded — and derived to
    /// something a directory listing can still be read.
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

    /// The name is the one thing in a listing that cannot be derived from the
    /// rest, and the title is what a caller sees first — so a guess slugged
    /// from the title has to land, and a guess that lands nowhere has to say
    /// what is there instead of only what is not.
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
        // The description is not searched: it is a sentence, and a sentence
        // matches too much for a command that also deletes.
        assert!(
            name("great-firewall").is_err(),
            "a fragment of the description is not a name"
        );

        // What must not happen: a guess that fits two memories picking one of
        // them, which for `rm` would be somebody else's fact deleted.
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

    /// Front matter survives a round trip, including a key this build does not
    /// know about — a rewrite must not quietly drop somebody's field.
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

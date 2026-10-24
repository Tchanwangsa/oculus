//! Keeping a bucket to a few pages: a line added to the memory it belongs to,
//! and several memories folded into one.

use std::path::Path;

use super::frontmatter::{parse, Entry};
use super::maintain::reindex;
use super::query::{find, list};
use super::slug::slug;
use super::write::{upsert, write, WriteSpec, Written};

/// What [`append`] did.
#[derive(Debug, serde::Serialize)]
pub struct Appended {
    #[serde(flatten)]
    pub written: Written,
    /// The line written, or the one removed when the replacement was empty.
    pub line: String,
    /// True when `replace` swapped or removed a line rather than adding one.
    pub replaced: bool,
}

/// Add one line to a memory, or swap the one line that contains `replace`
/// (an empty `line` removes it).
///
/// For the pages a subject keeps adding to — its gaps, an assessment — so a
/// new fact is a line where it belongs, not a file. A page that is not filed
/// yet is started when `about` and `kind` are given.
pub fn append(
    data_dir: &Path,
    code: Option<&str>,
    name: &str,
    line: &str,
    replace: Option<&str>,
    start: Option<(String, String)>,
) -> Result<Appended, String> {
    let line = bullet(line);
    let Some(entry) = filed(data_dir, code, name)? else {
        if replace.is_some() {
            return Err(format!("no memory called {name} to change a line in"));
        }
        let Some((about, kind)) = start else {
            return Err(format!(
                "no memory called {name} here — pass --about and --type to start it, \
                 or `oculus memory list` for the names filed"
            ));
        };
        if line.is_empty() {
            return Err("a new page needs its first line".into());
        }
        let written = write(
            data_dir,
            code,
            WriteSpec {
                name: Some(name.to_string()),
                description: Some(about),
                kind: Some(kind),
                body: Some(line.clone()),
                ..Default::default()
            },
        )?;
        return Ok(Appended {
            written,
            line,
            replaced: false,
        });
    };

    let text = std::fs::read_to_string(&entry.path)
        .map_err(|e| format!("cannot read {}: {e}", entry.path))?;
    let body = parse(&text).map(|(_, b)| b).unwrap_or(text);
    let (body, replaced, shown) = match replace {
        Some(fragment) => {
            let (body, old) = swap_line(&body, fragment, &line)?;
            (body, true, if line.is_empty() { old } else { line })
        }
        None if line.is_empty() => return Err("nothing to add — the line is empty".into()),
        None => {
            let body = body.trim_end();
            // A list continues; prose gets a blank line before the first bullet.
            let joint = if body.lines().last().is_some_and(is_bullet) {
                "\n"
            } else {
                "\n\n"
            };
            (format!("{body}{joint}{line}"), false, line)
        }
    };

    let written = write(
        data_dir,
        code,
        WriteSpec {
            name: Some(entry.front.name.clone()),
            body: Some(body),
            ..Default::default()
        },
    )?;
    Ok(Appended {
        written,
        line: shown,
        replaced,
    })
}

/// What [`fold`] did.
#[derive(Debug, serde::Serialize)]
pub struct Folded {
    #[serde(flatten)]
    pub written: Written,
    /// The memories deleted into it — every source but the target itself.
    pub removed: Vec<String>,
}

/// Write `spec` as one memory and delete `sources` into it.
///
/// The body is the caller's: a fold is a rewrite of what holds across the
/// sources, not their episodes pasted together. The result keeps the earliest
/// `created` among them, and a type they all share when none is passed.
pub fn fold(
    data_dir: &Path,
    code: Option<&str>,
    sources: &[String],
    mut spec: WriteSpec,
) -> Result<Folded, String> {
    let target = spec
        .name
        .as_deref()
        .map(slug)
        .filter(|s| !s.is_empty())
        .ok_or("--into names the memory they fold into")?;
    if spec.body.as_deref().is_none_or(|b| b.trim().is_empty()) {
        return Err(
            "a fold writes the merged memory — pass --text or --body with what \
                    holds across them, not the episodes"
                .into(),
        );
    }

    let mut entries: Vec<Entry> = Vec::new();
    for name in sources {
        let entry = find(data_dir, name, code)?;
        if entry.subject.as_deref() != code {
            return Err(format!(
                "{} is filed in {} — fold within one bucket, `oculus memory move` it first",
                entry.front.name,
                entry
                    .subject
                    .as_deref()
                    .unwrap_or("the cross-subject bucket")
            ));
        }
        if !entries.iter().any(|e| e.front.name == entry.front.name) {
            entries.push(entry);
        }
    }
    if entries.is_empty() {
        return Err("name the memories to fold".into());
    }
    let existing = filed(data_dir, code, &target)?;

    if spec.kind.is_none() && existing.is_none() {
        let kinds: Vec<&str> = entries
            .iter()
            .filter_map(|e| e.front.meta("type"))
            .collect();
        match kinds.first() {
            Some(first) if kinds.iter().all(|k| k == first) => {
                spec.kind = Some(first.to_string());
            }
            _ => return Err("they are of different types — pass --type".into()),
        }
    }
    let earliest = entries
        .iter()
        .chain(existing.iter())
        .filter_map(|e| e.dates().0)
        .min();
    if let Some(created) = earliest {
        spec.extra.push(("created".into(), created));
    }

    let mut written = upsert(data_dir, code, spec, false)?;
    let mut removed = Vec::new();
    for entry in entries.iter().filter(|e| e.front.name != target) {
        std::fs::remove_file(&entry.path)
            .map_err(|e| format!("cannot delete {}: {e}", entry.path))?;
        removed.push(entry.front.name.clone());
    }
    written.indexed = reindex(data_dir, code)?;
    Ok(Folded { written, removed })
}

/// The memory filed under exactly this name in this bucket, if there is one.
fn filed(data_dir: &Path, code: Option<&str>, name: &str) -> Result<Option<Entry>, String> {
    let wanted = slug(name);
    Ok(list(data_dir, code)?
        .into_iter()
        .find(|e| slug(&e.front.name) == wanted))
}

/// A line as a list item; empty stays empty.
fn bullet(line: &str) -> String {
    let line = line.trim();
    if line.is_empty() || is_bullet(line) {
        line.to_string()
    } else {
        format!("- {line}")
    }
}

fn is_bullet(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("- ") || t.starts_with("* ")
}

/// The body with the one line containing `fragment` replaced by `line`, or
/// removed when `line` is empty; and the line it took out.
fn swap_line(body: &str, fragment: &str, line: &str) -> Result<(String, String), String> {
    let needle = fragment.trim().to_lowercase();
    if needle.is_empty() {
        return Err("--replace wants some text from the line to change".into());
    }
    let hits: Vec<usize> = body
        .lines()
        .enumerate()
        .filter(|(_, l)| l.to_lowercase().contains(&needle))
        .map(|(i, _)| i)
        .collect();
    let at = match hits.as_slice() {
        [at] => *at,
        [] => return Err(format!("no line contains {fragment}")),
        many => {
            return Err(format!(
                "{} lines contain {fragment} — quote more of the one to change",
                many.len()
            ))
        }
    };
    let mut old = String::new();
    let mut out: Vec<&str> = Vec::new();
    for (i, l) in body.lines().enumerate() {
        if i == at {
            old = l.trim().to_string();
            if !line.is_empty() {
                out.push(line);
            }
        } else {
            out.push(l);
        }
    }
    Ok((out.join("\n"), old))
}

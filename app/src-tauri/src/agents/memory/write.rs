use std::path::Path;

use super::bucket::bucket_dir;
use super::frontmatter::{parse, render};
use super::maintain::{order_meta, reindex};
use super::slug::{name_from, slug};
use super::TYPES;

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
    let today = crate::runtime::clock::today_utc();
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

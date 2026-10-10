use std::path::Path;

use super::maintain::born;
use super::slug::humanize;

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

    pub(super) fn set_meta(&mut self, key: &str, value: &str) {
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

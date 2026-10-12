//! Reading `storyboard.md`, the free markdown the direct role writes: a
//! scene's entry starts at a `### <id>` heading (then the end of the line,
//! a space or `(`) and runs to the next `#`, `##` or `###` heading or a
//! `---` line. Headings and rules inside code fences don't count.

/// Scene `id`'s entry, verbatim from its heading line, trailing blank
/// lines dropped; `None` when no heading names it.
pub fn entry(src: &str, id: &str) -> Option<String> {
    let lines: Vec<&str> = src.lines().collect();
    let kinds = classify(&lines);
    let start = (0..lines.len()).find(|&i| kinds[i] && names(lines[i], id))?;
    let end = (start + 1..lines.len())
        .find(|&i| kinds[i] && ends_entry(lines[i]))
        .unwrap_or(lines.len());
    let body = &lines[start..end];
    let last = body.iter().rposition(|l| !l.trim().is_empty())?;
    Some(body[..=last].join("\n"))
}

/// What an entry's `Carries in:` and `Hands on:` say, each up to the other
/// label or the end of its line; `**` and `__` around the labels are
/// dropped.
pub fn handoff(entry: &str) -> (Option<String>, Option<String>) {
    (label(entry, "carries in:"), label(entry, "hands on:"))
}

/// The `notes/...` paths an entry mentions, in order, once each.
pub fn note_paths(entry: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut rest = entry;
    while let Some(at) = rest.find("notes/") {
        let preceded = rest[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '/' || c == '_' || c == '-');
        let tail = &rest[at..];
        let len = tail
            .find(|c: char| !(c.is_alphanumeric() || "._/-".contains(c)))
            .unwrap_or(tail.len());
        let path = tail[..len].trim_end_matches(['.', '/']);
        if !preceded && path.len() > "notes/".len() && !found.iter().any(|p| p == path) {
            found.push(path.to_owned());
        }
        rest = &tail[len.max(1)..];
    }
    found
}

/// For each line, whether it is outside a code fence (fence lines are
/// inside).
fn classify(lines: &[&str]) -> Vec<bool> {
    let mut open: Option<(char, usize)> = None;
    lines
        .iter()
        .map(|line| {
            let fence = fence(line);
            match (open, fence) {
                (None, Some(f)) => {
                    open = Some(f);
                    false
                }
                (Some((c, n)), Some((fc, fnn))) if c == fc && fnn >= n && is_bare(line) => {
                    open = None;
                    false
                }
                (Some(_), _) => false,
                (None, None) => true,
            }
        })
        .collect()
}

/// A fence opener: three or more backticks or tildes, indented at most 3.
fn fence(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start_matches(' ');
    if line.len() - t.len() > 3 {
        return None;
    }
    let c = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = t.chars().take_while(|x| *x == c).count();
    (n >= 3).then_some((c, n))
}

/// A closing fence carries nothing after its run.
fn is_bare(line: &str) -> bool {
    let t = line.trim();
    t.chars().all(|c| c == '`') || t.chars().all(|c| c == '~')
}

/// Whether `line` is `### <id>` then the end, a space or `(`.
fn names(line: &str, id: &str) -> bool {
    let Some(rest) = line.strip_prefix("###").and_then(|r| r.strip_prefix(' ')) else {
        return false;
    };
    let Some(after) = rest.trim_start().strip_prefix(id) else {
        return false;
    };
    after.trim_end().is_empty() || after.starts_with([' ', '('])
}

/// A `#` to `###` heading or a `---` rule.
fn ends_entry(line: &str) -> bool {
    let t = line.trim_end();
    let hashes = t.chars().take_while(|c| *c == '#').count();
    let heading = (1..=3).contains(&hashes) && (t.len() == hashes || t[hashes..].starts_with(' '));
    heading || (t.len() >= 3 && t.chars().all(|c| c == '-'))
}

fn label(entry: &str, name: &str) -> Option<String> {
    for line in entry.lines() {
        let clean = line.replace("**", "").replace("__", "");
        let lower = clean.to_ascii_lowercase();
        let Some(at) = lower.find(name) else {
            continue;
        };
        let start = at + name.len();
        let other = if name == "carries in:" {
            "hands on:"
        } else {
            "carries in:"
        };
        let end = lower[start..]
            .find(other)
            .map_or(clean.len(), |e| start + e);
        let text = clean[start..end].trim();
        return Some(text.to_owned());
    }
    None
}

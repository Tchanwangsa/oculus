//! The JSON in a model's reply. Models wrap it in prose, fences or an
//! envelope however plainly asked, so the jobs that want JSON back (lecture
//! chapters, the lecture end, a widget's `ask`) try every likely fragment.

/// The first fragment of `reply` that `decode` accepts, trying the whole
/// reply, each fenced block, then balanced `[…]`/`{…}` runs. `what` names the
/// expected shape in the error.
pub fn parse_reply<T>(
    reply: &str,
    what: &str,
    decode: impl Fn(&str) -> Option<T>,
) -> Result<T, String> {
    for candidate in json_candidates(reply) {
        if let Some(parsed) = decode(&candidate) {
            return Ok(parsed);
        }
    }
    Err(format!(
        "no {what} in the reply ({} chars): {}",
        reply.chars().count(),
        clip(reply.trim(), 200)
    ))
}

/// Fragments of `reply` worth trying to parse, most likely first.
fn json_candidates(reply: &str) -> Vec<String> {
    let mut out = vec![reply.trim().to_string()];
    // Fenced blocks; the opening fence's info string is dropped with its line.
    let mut rest = reply;
    while let Some(open) = rest.find("```") {
        let after = &rest[open + 3..];
        let body = match after.find('\n') {
            Some(nl) => &after[nl + 1..],
            None => break,
        };
        match body.find("```") {
            Some(close) => {
                out.push(body[..close].trim().to_string());
                rest = &body[close + 3..];
            }
            None => {
                out.push(body.trim().to_string());
                break;
            }
        }
    }
    for open in ['[', '{'] {
        out.extend(balanced_runs(reply, open));
    }
    out
}

/// Up to eight balanced `open`…`close` spans of `text`, in order — several,
/// because prose can hold brackets of its own. String literals are respected.
fn balanced_runs(text: &str, open: char) -> Vec<String> {
    const BALANCED_RUNS: usize = 8;
    let close = if open == '[' { ']' } else { '}' };
    let mut out: Vec<String> = Vec::new();
    let mut from = 0usize;
    while out.len() < BALANCED_RUNS {
        let Some(offset) = text[from..].find(open) else {
            break;
        };
        let start = from + offset;
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        let mut end: Option<usize> = None;
        for (at, ch) in text[start..].char_indices() {
            if in_string {
                match ch {
                    _ if escaped => escaped = false,
                    '\\' => escaped = true,
                    '"' => in_string = false,
                    _ => {}
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                c if c == open => depth += 1,
                c if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + at + ch.len_utf8());
                        break;
                    }
                }
                _ => {}
            }
        }
        match end {
            Some(e) => {
                out.push(text[start..e].to_string());
                from = e;
            }
            None => break,
        }
    }
    out
}

fn clip(text: &str, chars: usize) -> String {
    let head: String = text.chars().take(chars).collect();
    if text.chars().count() > chars {
        format!("{head}…")
    } else {
        head.to_string()
    }
}

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
pub(super) fn name_from(about: &str) -> String {
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

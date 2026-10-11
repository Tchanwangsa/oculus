//! "Did you mean" for unknown names: the closest candidate by edit distance,
//! ignoring case, when it is close enough to be a plausible typo.

/// The candidate closest to `name`, if any is within a third of its length.
pub fn closest<'a, I>(name: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    let lower = name.to_lowercase();
    let mut best: Option<(usize, &'a str)> = None;
    for c in candidates {
        let d = distance(&lower, &c.to_lowercase());
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, c));
        }
    }
    let limit = (name.chars().count() / 3).max(2);
    best.filter(|(d, _)| *d <= limit).map(|(_, c)| c)
}

/// " (did you mean `x`?)" or "".
pub fn did_you_mean<'a, I>(name: &str, candidates: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    closest(name, candidates)
        .map(|c| format!(" (did you mean `{c}`?)"))
        .unwrap_or_default()
}

/// Levenshtein distance over chars.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != *cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests;

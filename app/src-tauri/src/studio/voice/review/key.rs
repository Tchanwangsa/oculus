//! An item's place in the store: `<folder>/<key>.json`, the folder being
//! the subject's code or [`LIBRARY`], the key a file-safe form of the word.
//! The id the app passes around is `<folder>/<key>`.

/// The folder of a video with no subject; no subject code starts with `_`.
pub const LIBRARY: &str = "_library";

/// `word` with `’` read as `'`, lower-cased, and every character outside
/// `a-z`, `0-9` and `-` written `_<hex>_`, so two words never share a file.
pub fn key(word: &str) -> String {
    let mut out = String::new();
    for c in word.replace('’', "'").to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
            out.push(c);
        } else {
            out.push_str(&format!("_{:x}_", c as u32));
        }
    }
    out
}

/// The folder for `subject`; a code that cannot be a folder name is refused.
pub fn folder(subject: Option<&str>) -> Result<String, String> {
    match subject {
        None => Ok(LIBRARY.to_owned()),
        Some(code) if safe(code) && !code.starts_with('_') => Ok(code.to_owned()),
        Some(code) => Err(format!("subject {code:?} cannot name a folder")),
    }
}

/// `<folder>/<key>` for `word` under `subject`.
pub fn id(subject: Option<&str>, word: &str) -> Result<String, String> {
    Ok(format!("{}/{}", folder(subject)?, key(word)))
}

/// The folder and key of `id`, refusing anything else (a path that would
/// leave the store among them).
pub fn parse(id: &str) -> Result<(&str, &str), String> {
    let bad = || format!("{id:?} is not a pronunciation id");
    let (folder, key) = id.split_once('/').ok_or_else(bad)?;
    let key_ok = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if safe(folder) && key_ok {
        Ok((folder, key))
    } else {
        Err(bad())
    }
}

/// Letters, digits, `-`, `_` and `.`, not starting with `.`.
fn safe(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

//! Turning a video's burned-in captions on or off: video.json's top-level
//! `captions`, edited in the text so the rest of the file (the agent's
//! formatting, key order) stays as it was.

use std::path::Path;

/// Sets `<dir>/video.json`'s `captions` to `on`. Refuses a file that is not
/// a JSON object or whose `captions` is not `true` or `false`.
pub fn set_captions(dir: &Path, on: bool) -> Result<(), String> {
    let path = dir.join("video.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let edited = with_captions(&text, on).map_err(|e| format!("{}: {e}", path.display()))?;
    if edited != text {
        let tmp = dir.join(".video.json.tmp");
        crate::runtime::atomic_write::write(&path, &tmp, edited.as_bytes())?;
    }
    Ok(())
}

/// `text` with its top-level `captions` set to `on`: the literal replaced
/// where the key is, else the key added before `scenes` (on a line of its
/// own when `scenes` starts one), else after the opening brace.
pub fn with_captions(text: &str, on: bool) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("not valid JSON: {e}"))?;
    if !value.is_object() {
        return Err("video.json must be a JSON object".to_owned());
    }
    let literal = if on { "true" } else { "false" };
    let keys = top_level_keys(text);
    if let Some(&(_, end)) = keys.iter().find(|(k, _)| k == "\"captions\"") {
        let colon = end + text[end..].find(':').ok_or("no `:` after `captions`")?;
        let start = colon + 1 + (text[colon + 1..].len() - text[colon + 1..].trim_start().len());
        let old = ["true", "false"]
            .into_iter()
            .find(|v| text[start..].starts_with(v))
            .ok_or("`captions` must be true or false")?;
        return Ok(format!(
            "{}{literal}{}",
            &text[..start],
            &text[start + old.len()..]
        ));
    }
    let scenes = keys
        .iter()
        .find(|(k, _)| k == "\"scenes\"")
        .map(|(k, end)| end - k.len());
    let open = text.find('{').ok_or("no `{`")?;
    let at = scenes.unwrap_or(open + 1);
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let indent = &text[line_start..at];
    let insert = if scenes.is_some() && indent.trim().is_empty() {
        format!("\"captions\": {literal},\n{indent}")
    } else if scenes.is_some() {
        format!("\"captions\": {literal}, ")
    } else {
        format!(" \"captions\": {literal},")
    };
    let edited = format!("{}{insert}{}", &text[..at], &text[at..]);
    serde_json::from_str::<serde_json::Value>(&edited)
        .map_err(|e| format!("adding `captions` broke the file ({e}); add it by hand"))?;
    Ok(edited)
}

/// The keys of the top-level object, quoted, each with the byte just after
/// its closing quote.
fn top_level_keys(text: &str) -> Vec<(String, usize)> {
    let bytes = text.as_bytes();
    let mut keys = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth = depth.saturating_sub(1),
            b'"' => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                let end = (i + 1).min(bytes.len());
                let is_key = text[end..].trim_start().starts_with(':');
                if depth == 1 && is_key {
                    keys.push((text[start..end].to_owned(), end));
                }
                i = end;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    keys
}

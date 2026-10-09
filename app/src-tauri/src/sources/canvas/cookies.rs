//! Merging `Set-Cookie` values into the saved cookie header.

fn parse_cookie_header(header: &str) -> Vec<(String, String)> {
    header
        .split(';')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            let (name, value) = part.split_once('=')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

/// Merge `Set-Cookie` values into a cookie header, preserving order. `None`
/// when nothing changed, so the file is not rewritten on every request.
pub fn merged_cookie_header(current: &str, set_cookies: &[String]) -> Option<String> {
    if current.is_empty() {
        return None;
    }

    let mut pairs = parse_cookie_header(current);
    let mut changed = false;

    for raw in set_cookies {
        // "name=value; Path=/; HttpOnly" → we only care about the first pair.
        let Some(first) = raw.split(';').next() else {
            continue;
        };
        let Some((name, value)) = first.trim().split_once('=') else {
            continue;
        };
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty() {
            continue;
        }
        match pairs.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => {
                if slot.1 != value {
                    slot.1 = value.to_string();
                    changed = true;
                }
            }
            None => {
                pairs.push((name.to_string(), value.to_string()));
                changed = true;
            }
        }
    }

    changed.then(|| {
        pairs
            .iter()
            .map(|(n, v)| format!("{n}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    })
}

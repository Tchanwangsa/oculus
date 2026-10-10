/// Validate an ISO 8601 date and return the text unchanged — times are stored
/// as each source gives them (see `docs/calendar.md`). Accepts `YYYY-MM-DD`,
/// optionally plus `T`/space, `HH:MM[:SS[.fff]]` and `Z` or `±HH:MM`.
pub fn check_iso8601(value: &str) -> Result<String, String> {
    let text = value.trim();
    let bad =
        || format!("\"{text}\" is not an ISO 8601 date (want 2026-09-20 or 2026-09-20T23:59:00Z)");
    let bytes = text.as_bytes();
    let digits = |from: usize, n: usize| -> Option<u32> {
        let slice = text.get(from..from + n)?;
        if slice.len() == n && slice.bytes().all(|b| b.is_ascii_digit()) {
            slice.parse().ok()
        } else {
            None
        }
    };
    let in_range = |v: Option<u32>, lo: u32, hi: u32| v.filter(|v| *v >= lo && *v <= hi).is_some();

    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(bad());
    }
    if digits(0, 4).is_none() || !in_range(digits(5, 2), 1, 12) || !in_range(digits(8, 2), 1, 31) {
        return Err(bad());
    }
    if bytes.len() == 10 {
        return Ok(text.to_string());
    }
    if !matches!(bytes[10], b'T' | b't' | b' ') || bytes.len() < 16 || bytes[13] != b':' {
        return Err(bad());
    }
    if !in_range(digits(11, 2), 0, 23) || !in_range(digits(14, 2), 0, 59) {
        return Err(bad());
    }
    let mut i = 16;
    if bytes.get(i) == Some(&b':') {
        if !in_range(digits(i + 1, 2), 0, 60) {
            return Err(bad());
        }
        i += 3;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while bytes.get(i).is_some_and(|b| b.is_ascii_digit()) {
            i += 1;
        }
        if i == start {
            return Err(bad());
        }
    }
    match bytes.get(i) {
        None => Ok(text.to_string()),
        Some(b'Z') | Some(b'z') if i + 1 == bytes.len() => Ok(text.to_string()),
        Some(b'+') | Some(b'-') => {
            let rest = &text[i + 1..];
            let ok = match rest.len() {
                5 => {
                    rest.as_bytes()[2] == b':'
                        && in_range(digits(i + 1, 2), 0, 23)
                        && in_range(digits(i + 4, 2), 0, 59)
                }
                4 => in_range(digits(i + 1, 2), 0, 23) && in_range(digits(i + 3, 2), 0, 59),
                _ => false,
            };
            if ok {
                Ok(text.to_string())
            } else {
                Err(bad())
            }
        }
        _ => Err(bad()),
    }
}

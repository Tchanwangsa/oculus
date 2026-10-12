//! Pause markers in script.md: `[pause]` holds the picture for the theme's
//! `narration.pause` seconds, `[pause 1.5]` for that many. They are not
//! said. A bracket opening with `pause` that is neither form is malformed:
//! it is dropped from the spoken text too, and lint names it.

/// The longest pause a marker or a theme may ask for, in seconds.
pub const MAX_PAUSE: f64 = 5.0;

/// A `[pause …]` in one line of script.md.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    /// Byte range in the line, brackets included.
    pub start: usize,
    pub end: usize,
    /// Its seconds (`None`: the theme's), or why it is malformed.
    pub seconds: Result<Option<f64>, String>,
}

/// Every pause marker in `line`, in order. A `[` whose text starts with
/// `pause` is one, up to the next `]` or the line's end.
pub fn markers(line: &str) -> Vec<Marker> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(open) = line[from..].find('[').map(|i| from + i) {
        let inner_from = open + 1;
        let is_pause = line[inner_from..]
            .trim_start()
            .get(..5)
            .is_some_and(|w| w.eq_ignore_ascii_case("pause"));
        if !is_pause {
            from = inner_from;
            continue;
        }
        let close = line[inner_from..].find(']').map(|i| inner_from + i);
        let end = close.map_or(line.len(), |c| c + 1);
        let written = &line[open..end];
        let seconds = match close {
            None => Err(format!("`{written}` has no closing `]`")),
            Some(c) => seconds(&line[inner_from..c], written),
        };
        out.push(Marker {
            start: open,
            end,
            seconds,
        });
        from = end;
    }
    out
}

/// What `pause` or `pause <seconds>` asks for.
fn seconds(inner: &str, written: &str) -> Result<Option<f64>, String> {
    let shape = || {
        format!(
            "`{written}` is not a pause; write `[pause]` for the theme's pause or `[pause 1.5]` for seconds"
        )
    };
    let mut words = inner.split_whitespace();
    if !words
        .next()
        .is_some_and(|w| w.eq_ignore_ascii_case("pause"))
    {
        return Err(shape());
    }
    let Some(n) = words.next() else {
        return Ok(None);
    };
    let decimal = n.chars().all(|c| c.is_ascii_digit() || c == '.')
        && n.chars().filter(|&c| c == '.').count() <= 1
        && n.chars().any(|c| c.is_ascii_digit());
    let value: f64 = match (decimal, words.next()) {
        (true, None) => n.parse().map_err(|_| shape())?,
        _ => return Err(shape()),
    };
    if !(value > 0.0 && value <= MAX_PAUSE) {
        return Err(format!(
            "`{written}`: a pause is more than 0 and at most {MAX_PAUSE} seconds"
        ));
    }
    Ok(Some(value))
}

/// `seconds` as a marker writes it: `1`, `1.5`.
pub(crate) fn written(seconds: f64) -> String {
    format!("[pause {seconds}]")
}

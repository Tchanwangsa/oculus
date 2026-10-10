//! The reply: parsed leniently, then checked against the transcript.

use super::window::Line;

/// What the model said, before validation.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub ends_at: Option<f64>,
    pub quote: Option<String>,
}

/// The `{ends_at, quote}` object out of whatever the model said: prose,
/// fences and an envelope around it are tolerated, as in `chapters`.
pub fn parse_reply(reply: &str) -> Result<Reply, String> {
    crate::lectures::chapters::parse_reply(reply, "{\"ends_at\", \"quote\"} object", decode)
}

fn decode(text: &str) -> Option<Reply> {
    use serde_json::Value;
    let value: Value = serde_json::from_str(text).ok()?;
    let object = value.as_object()?;
    let key = |o: &serde_json::Map<String, Value>| {
        ["ends_at", "endsAt", "end"]
            .into_iter()
            .find(|k| o.contains_key(*k))
    };
    // An envelope: the one object inside that has the key.
    let object = match key(object) {
        Some(_) => object,
        None => object
            .values()
            .filter_map(Value::as_object)
            .find(|o| key(o).is_some())?,
    };
    let ends_at = match &object[key(object)?] {
        Value::Null => None,
        Value::Number(n) => Some(n.as_f64()?),
        // "1911 31:51": the first column copied with the clock beside it.
        Value::String(s) => Some(s.split_whitespace().next()?.parse::<f64>().ok()?),
        _ => return None,
    };
    let quote = match object.get("quote") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        Some(_) => return None,
    };
    Some(Reply { ends_at, quote })
}

/// A validated end.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// The start second of the line the model cited.
    pub cue_start: u32,
    /// What is stored: the end of the line the quote finishes in.
    pub end: u32,
    pub quote: String,
}

/// Lowercase words: punctuation becomes a space, whitespace collapses, so
/// "I'll stop here." and "i ll stop here" compare equal.
fn normalise(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Where `quote` (normalised) occurs touching line `at`, with the lines
/// either side joined on: the index of the last line it reaches.
fn quote_ends_in(lines: &[Line], at: usize, quote: &str) -> Option<usize> {
    let first = at.saturating_sub(1);
    let last = (at + 1).min(lines.len() - 1);
    let mut haystack = String::from(" ");
    let mut spans = Vec::new();
    for index in first..=last {
        let start = haystack.len();
        haystack.push_str(&normalise(&lines[index].text));
        spans.push((index, start, haystack.len()));
        haystack.push(' ');
    }
    let needle = format!(" {quote} ");
    let mut from = 0;
    while let Some(offset) = haystack[from..].find(&needle) {
        let begin = from + offset + 1;
        let end = begin + quote.len();
        let touched: Vec<usize> = spans
            .iter()
            .filter(|(_, s, e)| begin < *e && end > *s)
            .map(|(index, ..)| *index)
            .collect();
        if touched.contains(&at) {
            return touched.last().copied();
        }
        from = begin;
    }
    None
}

/// How far from the cited second the quote may sit. Small models cite the
/// line before the one they quote; the quote, not the second, is the evidence.
const NEAR_SECS: u32 = 30;

/// Whether a reply may be stored. The quote must be words of a line starting
/// within [`NEAR_SECS`] of `ends_at` (or run from it into the next line); the
/// nearest such line wins. Null for both is "no end".
pub fn validate(reply: &Reply, lines: &[Line]) -> Result<Option<Found>, String> {
    let (at, quote) = match (reply.ends_at, &reply.quote) {
        (None, None) => return Ok(None),
        (None, Some(_)) => {
            return Err(
                "ends_at is null but a quote was given — reply null for both, or cite the line"
                    .into(),
            )
        }
        (Some(at), None) => {
            return Err(format!(
                "ends_at {at} has no quote — copy 3 to 12 words from that line"
            ))
        }
        (Some(at), Some(quote)) => (at, quote),
    };
    let (first, last) = match (lines.first(), lines.last()) {
        (Some(f), Some(l)) => (f.start, l.end),
        _ => return Err("there is no transcript to cite".into()),
    };
    if !at.is_finite() || at < first as f64 || at > last as f64 {
        return Err(format!(
            "ends_at {at} is not a second of the transcript shown"
        ));
    }
    let second = at.round() as u32;
    let words = normalise(quote);
    if words.is_empty() {
        return Err(format!("the quote {quote:?} has no words in it"));
    }
    let mut near: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].start.abs_diff(second) <= NEAR_SECS)
        .collect();
    near.sort_by_key(|&i| lines[i].start.abs_diff(second));
    for index in near {
        if let Some(last) = quote_ends_in(lines, index, &words) {
            return Ok(Some(Found {
                cue_start: lines[index].start,
                end: lines[last].end,
                quote: quote.clone(),
            }));
        }
    }
    let elsewhere = (0..lines.len()).find(|&i| quote_ends_in(lines, i, &words).is_some());
    Err(match elsewhere {
        Some(i) => format!(
            "the quote {quote:?} is not near {second}; it is in the line at {}",
            lines[i].start
        ),
        None => format!(
            "the quote {quote:?} is not in the transcript near {second} — copy the words exactly"
        ),
    })
}

/// One turn, parsed and validated; a rejected reply is asked again once with
/// the reason appended. `turn` is the agent call (a fake one in tests). A
/// provider failure is not retried.
pub fn ask(
    mut turn: impl FnMut(&str) -> Result<String, String>,
    prompt: &str,
    lines: &[Line],
) -> Result<Option<Found>, String> {
    let mut failure = String::new();
    for attempt in 0..2 {
        let text = if attempt == 0 {
            prompt.to_string()
        } else {
            format!(
                "{prompt}\nYour previous reply was rejected: {failure}\n\
                 Reply again with only the JSON object."
            )
        };
        let reply = turn(&text)?;
        match parse_reply(&reply).and_then(|r| validate(&r, lines)) {
            Ok(found) => return Ok(found),
            Err(e) => failure = e,
        }
    }
    Err(format!("the reply was rejected twice: {failure}"))
}

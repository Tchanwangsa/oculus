//! A CSV as a one-sheet section, and the cell formatting a workbook shares.

use calamine::{Data, ExcelDateTime};

use super::workbook::table;

/// A CSV is one sheet, named after its file. Text that isn't UTF-8 is read as
/// Latin-1, which is what a CSV exported on Windows usually is.
pub(super) fn csv_section(filename: &str, bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    };
    let rows: Vec<Vec<String>> = csv_rows(&text, delimiter(&text))
        .into_iter()
        .map(|row| row.iter().map(|c| escape(c)).collect())
        .collect();
    let body = table(&rows).unwrap_or_else(|| "(empty)".to_string());
    format!(
        "## {}

{body}",
        filename.trim()
    )
}

/// Whichever of `,` `;` tab or `|` the first line uses most: Excel writes `;`
/// in locales whose decimal mark is a comma.
pub(super) fn delimiter(text: &str) -> char {
    let first = text.lines().next().unwrap_or("");
    let count = |d: char| first.chars().filter(|&c| c == d).count();
    [',', ';', '\t', '|']
        .into_iter()
        .max_by_key(|&d| (count(d), d == ','))
        .unwrap_or(',')
}

/// RFC 4180 rows: quoted fields may hold the delimiter, line breaks and `""`
/// for a quote; CRLF and LF both end a row.
pub(super) fn csv_rows(text: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
        } else if c == '"' && field.is_empty() {
            quoted = true;
        } else if c == delimiter {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// A cell as it reads in the sheet, safe inside a table row.
pub(super) fn cell(value: &Data) -> String {
    let text = match value {
        Data::Empty => return String::new(),
        Data::String(s) => s.clone(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => number(*f),
        Data::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        Data::DateTime(d) => date_time(d),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => e.to_string(),
    };
    escape(&text)
}

/// Text made safe inside a table row: trimmed, line breaks as `<br>`, pipes
/// escaped.
pub(super) fn escape(text: &str) -> String {
    text.trim()
        .replace("\r\n", "<br>")
        .replace(['\n', '\r'], "<br>")
        .replace('|', "\\|")
}

/// Whole numbers without a decimal point, others at the 15 significant digits
/// Excel keeps, which drops binary noise such as `0.30000000000000004`.
fn number(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return (value as i64).to_string();
    }
    format!("{value:.14e}")
        .parse::<f64>()
        .unwrap_or(value)
        .to_string()
}

/// ISO dates: `2026-03-02`, `2026-03-02 09:30:00`, or `09:30:00` for a time
/// of day; a duration as `h:mm:ss`.
fn date_time(value: &ExcelDateTime) -> String {
    let serial = value.as_f64();
    if value.is_duration() {
        let seconds = (serial * 86_400.0).round() as i64;
        return format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds.abs() % 3600 / 60,
            seconds.abs() % 60
        );
    }
    let (y, mo, d, h, mi, s, _) = value.to_ymd_hms_milli();
    let date = format!("{y:04}-{mo:02}-{d:02}");
    let time = format!("{h:02}:{mi:02}:{s:02}");
    if serial.fract() == 0.0 {
        date
    } else if serial < 1.0 {
        time
    } else {
        format!("{date} {time}")
    }
}

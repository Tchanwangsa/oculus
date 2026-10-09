//! Spreadsheets as text. A workbook (xlsx, xlsm, xls, ods) is read in-process
//! with calamine and written as `{name}.md` beside it: `# {name}`, then one
//! `##` section per worksheet holding its used range as a GFM table of values
//! (merged blocks filled, `=FORMULA` where no result was stored), then its
//! formulas, one line per copied-down or -across pattern. Each sheet is one
//! `pages` row (page 1 is the first sheet), so `oculus read`, `oculus grep`
//! and the lexical index see it. Nothing is sent to a parse service and
//! nothing is embedded: a table has no page image worth ranking.
//! See `docs/parsing.md`.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Seek};
use std::path::Path;

use calamine::{CellType, Data, Dimensions, ExcelDateTime, Range, Reader, SheetType, Sheets};
use sqlx::{Row, SqlitePool};

use crate::parse::{self, ParseError, ParsePage};
use crate::paths;

/// `{rel}.md`: where a spreadsheet's text lives.
pub fn md_rel(rel: &str) -> String {
    format!("{rel}.md")
}

/// The PDF route's files beside a spreadsheet — a converted `.pdf`, its parse
/// record, images or vectors. Text derived from a PDF is not this module's,
/// so their presence means the sheet is converted again.
fn pdf_route_files(data_dir: &Path, rel: &str) -> bool {
    [".pdf", ".pages.json", ".emb.json", "_images"]
        .iter()
        .any(|suffix| data_dir.join(format!("{rel}{suffix}")).exists())
}

/// True when `rel`'s text is missing or sits beside PDF-route files.
pub fn needs_conversion(data_dir: &Path, rel: &str) -> bool {
    !data_dir.join(md_rel(rel)).is_file() || pdf_route_files(data_dir, rel)
}

// ── Workbook → markdown ──────────────────────────────────────────────────────

/// One `## sheet` section per worksheet, in workbook order. Chart, dialog and
/// macro sheets hold no cells and are left out.
pub fn sections(bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut book = calamine::open_workbook_auto_from_rs(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let names: Vec<String> = book
        .sheets_metadata()
        .iter()
        .filter(|sheet| sheet.typ == SheetType::WorkSheet)
        .map(|sheet| sheet.name.clone())
        .collect();
    names
        .iter()
        .map(|name| {
            let failed = |e: String| format!("sheet {name}: {e}");
            let values = book.worksheet_range(name).map_err(|e| failed(e.to_string()))?;
            let formulas = book.worksheet_formula(name).map_err(|e| failed(e.to_string()))?;
            let merges = merged_regions(&mut book, name).map_err(failed)?;
            Ok(section(name, &values, &formulas, &merges))
        })
        .collect()
}

/// A sheet's merged regions in absolute sheet coordinates. xlsx and xls
/// record them; calamine reads none from ods or xlsb, so there a block keeps
/// its value in its top-left cell only.
fn merged_regions<RS: Read + Seek>(book: &mut Sheets<RS>, name: &str) -> Result<Vec<Dimensions>, String> {
    match book {
        Sheets::Xlsx(xlsx) => xlsx.merge_cells_by_sheet_name(name).map_err(|e| e.to_string()),
        Sheets::Xls(xls) => xls.merge_cells_by_sheet_name(name).map_err(|e| e.to_string()),
        Sheets::Xlsb(_) | Sheets::Ods(_) => Ok(Vec::new()),
    }
}

/// The whole file: the workbook's name, then its sections.
pub fn document(filename: &str, sections: &[String]) -> String {
    let mut out = format!("# {filename}\n");
    for section in sections {
        out.push('\n');
        out.push_str(section);
        out.push('\n');
    }
    out
}

/// The sheet's table of values, then its formulas when it has any.
fn section(name: &str, values: &Range<Data>, formulas: &Range<String>, merges: &[Dimensions]) -> String {
    let mut texts = texts(values, formulas);
    fill_merges(&mut texts, merges);
    let rows: Vec<Vec<String>> = texts.rows().map(<[String]>::to_vec).collect();
    let body = table(&rows).unwrap_or_else(|| "(empty)".to_string());
    let mut out = format!("## {}\n\n{body}", name.trim());
    if let Some(list) = formula_list(formulas) {
        out.push_str("\n\nFormulas:\n");
        out.push_str(&list);
    }
    out
}

/// Each cell as table text, over both ranges (they can start apart): its
/// value, or `=FORMULA` where the file stored no result for a formula — a
/// workbook written by a script and never recalculated has none.
fn texts(values: &Range<Data>, formulas: &Range<String>) -> Range<String> {
    let mut extents = [values.start().zip(values.end()), formulas.start().zip(formulas.end())]
        .into_iter()
        .flatten();
    let Some(first) = extents.next() else { return Range::empty() };
    let (start, end) = extents.fold(first, |(s, e), (s2, e2)| {
        ((s.0.min(s2.0), s.1.min(s2.1)), (e.0.max(e2.0), e.1.max(e2.1)))
    });
    let mut out = Range::new(start, end);
    let origin = values.start().unwrap_or_default();
    for (row, col, value) in values.used_cells() {
        out.set_value((origin.0 + row as u32, origin.1 + col as u32), cell(value));
    }
    let origin = formulas.start().unwrap_or_default();
    for (row, col, raw) in formulas.used_cells() {
        let pos = (origin.0 + row as u32, origin.1 + col as u32);
        let stored = values.get_value(pos).is_some_and(|v| *v != Data::Empty);
        if let Some(body) = formula_body(raw).filter(|_| !stored) {
            out.set_value(pos, escape(&format!("={body}")));
        }
    }
    out
}

/// Copy each merged region's top-left value over the rest of its block, so
/// every row reads on its own (a rubric's section label, a grouped header).
/// `merges` are absolute sheet coordinates; parts outside `range` are dropped.
fn fill_merges<T: CellType>(range: &mut Range<T>, merges: &[Dimensions]) {
    let Some(end) = range.end() else { return };
    for region in merges {
        let Some(value) = range.get_value(region.start).filter(|v| **v != T::default()).cloned() else {
            continue;
        };
        for row in region.start.0..=region.end.0.min(end.0) {
            for col in region.start.1..=region.end.1.min(end.1) {
                range.set_value((row, col), value.clone());
            }
        }
    }
}

/// A GFM table of the rows' non-empty extent, first row as the header; `None`
/// when no cell holds anything. A run of blank rows inside it is kept as one,
/// which still separates the groups a sheet's layout drew.
fn table(rows: &[Vec<String>]) -> Option<String> {
    let filled = |row: &Vec<String>| row.iter().any(|c| !c.is_empty());
    let first = rows.iter().position(filled)?;
    let last = rows.iter().rposition(filled)?;
    let rows: Vec<&Vec<String>> = rows[first..=last]
        .iter()
        .enumerate()
        .filter(|&(i, row)| filled(row) || filled(&rows[first + i - 1]))
        .map(|(_, row)| row)
        .collect();
    let column_filled =
        |i: usize| rows.iter().any(|row| row.get(i).is_some_and(|c| !c.is_empty()));
    let width = rows.iter().map(|row| row.len()).max().unwrap_or(0);
    let left = (0..width).find(|&i| column_filled(i))?;
    let right = (0..width).rev().find(|&i| column_filled(i))?;

    let line = |row: &&Vec<String>| {
        let cells: Vec<&str> =
            (left..=right).map(|i| row.get(i).map(String::as_str).unwrap_or("")).collect();
        format!("| {} |", cells.join(" | "))
    };
    let mut out = vec![line(&rows[0])];
    out.push(format!("|{}", " --- |".repeat(right - left + 1)));
    out.extend(rows[1..].iter().map(line));
    Some(out.join("\n"))
}

/// A cell as it reads in the sheet, safe inside a table row.
fn cell(value: &Data) -> String {
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
fn escape(text: &str) -> String {
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
    format!("{value:.14e}").parse::<f64>().unwrap_or(value).to_string()
}

/// ISO dates: `2026-03-02`, `2026-03-02 09:30:00`, or `09:30:00` for a time
/// of day; a duration as `h:mm:ss`.
fn date_time(value: &ExcelDateTime) -> String {
    let serial = value.as_f64();
    if value.is_duration() {
        let seconds = (serial * 86_400.0).round() as i64;
        return format!("{}:{:02}:{:02}", seconds / 3600, seconds.abs() % 3600 / 60, seconds.abs() % 60);
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

// ── Formulas ─────────────────────────────────────────────────────────────────

/// Lines listed under one sheet's table before the rest are counted.
const FORMULA_LINES: usize = 200;
/// Blocks named on one line before the rest are counted.
const LINE_BLOCKS: usize = 20;

/// A formula as written, without ods's `of:` namespace or a leading `=`;
/// `None` for none, and for the placeholder calamine writes for an xls
/// formula it cannot decode.
fn formula_body(raw: &str) -> Option<&str> {
    let body = raw.trim();
    let body = body.strip_prefix("of:").unwrap_or(body);
    let body = body.strip_prefix('=').unwrap_or(body).trim();
    (!body.is_empty() && !body.starts_with("Unrecognised formula")).then_some(body)
}

/// The sheet's formulas as a bullet list, one line per pattern in order of
/// its first cell: cells whose formulas match once references are made
/// relative ([`relative`]) — a run copied down or across — share a line
/// naming the blocks they cover and the first cell's formula. `None` when the
/// sheet has none.
fn formula_list(formulas: &Range<String>) -> Option<String> {
    let origin = formulas.start()?;
    let mut patterns: Vec<(&str, Vec<(u32, u32)>)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    // Row-major, so a pattern's first cell decides its place.
    for (row, col, raw) in formulas.used_cells() {
        let Some(body) = formula_body(raw) else { continue };
        let pos = (origin.0 + row as u32, origin.1 + col as u32);
        let at = *index.entry(relative(body, pos)).or_insert_with(|| {
            patterns.push((body, Vec::new()));
            patterns.len() - 1
        });
        patterns[at].1.push(pos);
    }
    if patterns.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = patterns
        .iter()
        .take(FORMULA_LINES)
        .map(|(body, cells)| {
            let blocks = blocks(cells);
            let mut names: Vec<String> =
                blocks.iter().take(LINE_BLOCKS).map(|&(a, b)| block_name(a, b)).collect();
            if blocks.len() > LINE_BLOCKS {
                names.push(format!("… and {} more", blocks.len() - LINE_BLOCKS));
            }
            let body = body.replace("\r\n", " ").replace(['\n', '\r'], " ");
            format!("- {}: ={body}", names.join(", "))
        })
        .collect();
    if patterns.len() > FORMULA_LINES {
        lines.push(format!("- … and {} more", patterns.len() - FORMULA_LINES));
    }
    Some(lines.join("\n"))
}

/// Cover `cells` (row-major) with rectangles, each the larger of the one
/// reached by running down from its first cell and then widening, or by
/// running across and then deepening.
fn blocks(cells: &[(u32, u32)]) -> Vec<((u32, u32), (u32, u32))> {
    let mut left: HashSet<(u32, u32)> = cells.iter().copied().collect();
    let mut out = Vec::new();
    for &(row, col) in cells {
        if !left.contains(&(row, col)) {
            continue;
        }
        let down_first = {
            let mut bottom = row;
            while left.contains(&(bottom + 1, col)) {
                bottom += 1;
            }
            let mut right = col;
            while (row..=bottom).all(|r| left.contains(&(r, right + 1))) {
                right += 1;
            }
            (bottom, right)
        };
        let across_first = {
            let mut right = col;
            while left.contains(&(row, right + 1)) {
                right += 1;
            }
            let mut bottom = row;
            while (col..=right).all(|c| left.contains(&(bottom + 1, c))) {
                bottom += 1;
            }
            (bottom, right)
        };
        let area = |(bottom, right): (u32, u32)| (bottom - row + 1) as u64 * (right - col + 1) as u64;
        let end = if area(across_first) > area(down_first) { across_first } else { down_first };
        for r in row..=end.0 {
            for c in col..=end.1 {
                left.remove(&(r, c));
            }
        }
        out.push(((row, col), end));
    }
    out
}

/// `D9`, or `D9:E14` for a block.
fn block_name(start: (u32, u32), end: (u32, u32)) -> String {
    if start == end {
        cell_name(start)
    } else {
        format!("{}:{}", cell_name(start), cell_name(end))
    }
}

/// A zero-based (row, column) as an A1 name.
fn cell_name((row, col): (u32, u32)) -> String {
    format!("{}{}", column_name(col), row + 1)
}

/// A zero-based column as letters: 0 is `A`, 26 is `AA`.
fn column_name(col: u32) -> String {
    let mut n = col + 1;
    let mut letters = Vec::new();
    while n > 0 {
        n -= 1;
        letters.push(b'A' + (n % 26) as u8);
        n /= 26;
    }
    letters.iter().rev().map(|&b| b as char).collect()
}

/// `formula` with each A1 reference rewritten relative to `at`, the cell it
/// sits in — `G9` in D9 reads `R[0]C[3]`, `$G$9` reads `R9C7` — so copies of
/// one formula down or across compare equal. "Strings", 'quoted' sheet names,
/// function names and `Sheet!` prefixes are kept as written.
fn relative(formula: &str, at: (u32, u32)) -> String {
    let bytes = formula.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || !b.is_ascii();
    let mut out = String::with_capacity(formula.len() + 16);
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let end = if b == b'"' || b == b'\'' {
            // A doubled quote inside is an escaped one and closes nothing.
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b && bytes.get(j + 1) == Some(&b) {
                    j += 2;
                } else if bytes[j] == b {
                    j += 1;
                    break;
                } else {
                    j += 1;
                }
            }
            out.push_str(&formula[i..j]);
            j
        } else if word(b) {
            let mut j = i;
            while j < bytes.len() && word(bytes[j]) {
                j += 1;
            }
            let token = &formula[i..j];
            // `LOG10(` is a function and `A1!` a sheet name, not references.
            let reference = match bytes.get(j) {
                Some(b'(' | b'!') => None,
                _ => reference(token),
            };
            match reference {
                Some((row_abs, row, col_abs, col)) => {
                    let part = |abs: bool, n: u32, here: u32, axis: char| {
                        if abs {
                            format!("{axis}{}", n + 1)
                        } else {
                            format!("{axis}[{}]", n as i64 - here as i64)
                        }
                    };
                    out.push_str(&part(row_abs, row, at.0, 'R'));
                    out.push_str(&part(col_abs, col, at.1, 'C'));
                }
                None => out.push_str(token),
            }
            j
        } else {
            out.push(b as char);
            i + 1
        };
        i = end;
    }
    out
}

/// An A1 reference such as `$G9` as (row absolute, zero-based row, column
/// absolute, zero-based column); `None` for any other word or one past the
/// sheet's bounds.
fn reference(token: &str) -> Option<(bool, u32, bool, u32)> {
    let col_abs = token.starts_with('$');
    let rest = &token[col_abs as usize..];
    let letters = rest.bytes().take_while(u8::is_ascii_uppercase).count();
    if !(1..=3).contains(&letters) {
        return None;
    }
    let col = rest[..letters].bytes().fold(0u32, |n, b| n * 26 + (b - b'A' + 1) as u32) - 1;
    let rest = &rest[letters..];
    let row_abs = rest.starts_with('$');
    let digits = &rest[row_abs as usize..];
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let row: u32 = digits.parse().ok()?;
    (col < 16_384 && row <= 1_048_576).then_some((row_abs, row - 1, col_abs, col))
}

// ── On disk ──────────────────────────────────────────────────────────────────

/// Convert the spreadsheet at `rel` and write its `.md`, returning one page
/// per worksheet. Whatever text or PDF-route files were beside it go first,
/// so a workbook that no longer reads leaves no stale text behind.
pub fn convert(data_dir: &Path, rel: &str) -> Result<Vec<ParsePage>, ParseError> {
    let source = data_dir.join(rel);
    let bytes = std::fs::read(&source)
        .map_err(|e| ParseError::Io(format!("read {}: {e}", source.display())))?;
    paths::purge_parse_artifacts(data_dir, rel);

    let sections = sections(&bytes).map_err(|detail| {
        // The UI keeps the sentence; calamine's reason goes to stderr.
        eprintln!("[oculus] spreadsheet unreadable: {rel}: {detail}");
        ParseError::Document { code: parse::SHEET_UNREADABLE.into() }
    })?;
    let filename = rel.rsplit('/').next().unwrap_or(rel);
    let md = data_dir.join(md_rel(rel));
    let tmp = md.with_extension(format!("md.tmp{}-{}", std::process::id(), crate::clock::now_nanos()));
    crate::atomic_write::write(&md, &tmp, document(filename, &sections).as_bytes())
        .map_err(ParseError::Io)?;

    Ok(sections
        .into_iter()
        .enumerate()
        .map(|(i, markdown)| ParsePage { page_no: i as u32 + 1, markdown })
        .collect())
}

// ── In the database ──────────────────────────────────────────────────────────

/// Store a converted sheet's pages and mark it finished, as a parse would.
/// Its pages are replaced outright and it has no vectors, so the embed
/// columns are cleared.
///
/// A sync converts straight after its `scrape-file` event, which can beat the
/// frontend's write of the row, so a bare row is inserted here when there is
/// none; the frontend's upsert fills in the rest.
pub async fn record(
    pool: &SqlitePool,
    subject_id: i64,
    rel: &str,
    pages: &[ParsePage],
) -> Result<usize, String> {
    let filename = rel.rsplit('/').next().unwrap_or(rel);
    let file_type = filename.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    sqlx::query(
        "INSERT INTO files (subject_id, filename, relative_path, file_type, first_seen_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'))
         ON CONFLICT(subject_id, relative_path) DO NOTHING",
    )
    .bind(subject_id)
    .bind(filename)
    .bind(rel)
    .bind(file_type)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some(file_id) = crate::store::file_id(pool, subject_id, rel).await? else {
        return Ok(0);
    };
    let recorded = crate::store::replace_pages(pool, file_id, pages).await?;
    sqlx::query(
        "UPDATE files SET parse_status = 'quality', parsed_at = datetime('now'),
                          embed_status = NULL, embedded_at = NULL
          WHERE id = ?1",
    )
    .bind(file_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(recorded)
}

/// A sheet that did not convert has no text: its pages go and it reads as
/// failed until its bytes change or it is retried.
async fn forget(pool: &SqlitePool, subject_id: i64, rel: &str) -> Result<(), String> {
    let Some(file_id) = crate::store::file_id(pool, subject_id, rel).await? else {
        return Ok(());
    };
    sqlx::query("DELETE FROM pages WHERE file_id = ?1")
        .bind(file_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE files SET parse_status = 'error', parsed_at = NULL,
                          embed_status = NULL, embedded_at = NULL
          WHERE id = ?1",
    )
    .bind(file_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Convert, record and report one spreadsheet, blocking (it takes moments).
/// Ends in the `parse-status` a parse ends in — `quality`, or `error` with a
/// `Document` failure — so the pipeline row settles; no parse is started.
/// Call from a plain thread: it blocks on the async runtime.
pub fn index(data_dir: &Path, rel: &str, subject_id: i64) -> Result<usize, ParseError> {
    let converted = convert(data_dir, rel);
    let stored = tauri::async_runtime::block_on(async {
        let pool = crate::store::open(data_dir).await?;
        let result = match &converted {
            Ok(pages) => record(&pool, subject_id, rel, pages).await,
            Err(_) => forget(&pool, subject_id, rel).await.map(|()| 0),
        };
        pool.close().await;
        result
    });
    // A database problem must not fail a conversion that is on disk; the
    // startup reconcile records it later.
    let recorded = stored.unwrap_or_else(|e| {
        eprintln!("[oculus] spreadsheet {rel}: not recorded: {e}");
        0
    });
    match converted {
        Ok(_) => {
            parse::events::parsed(rel, subject_id);
            Ok(recorded)
        }
        Err(error) => {
            parse::events::failed(rel, subject_id, &error);
            Err(error)
        }
    }
}

/// Bring every spreadsheet on record to its text: one with no text, PDF-route
/// files beside it, no pages, vectors or an unfinished status is converted
/// again. A failure stays failed unless PDF-route files are still there, and
/// a skip stays skipped.
/// Silent — the frontend reads the rows. Returns how many were converted.
pub async fn reconcile(pool: &SqlitePool, data_dir: &Path) -> Result<u64, String> {
    let rows = sqlx::query(
        "SELECT f.subject_id, f.relative_path, f.parse_status, f.embed_status,
                (SELECT COUNT(*) FROM pages p WHERE p.file_id = f.id) AS pages
           FROM files f",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut converted = 0;
    for row in &rows {
        let rel: String = row.get("relative_path");
        if !paths::is_sheet(&rel) || !data_dir.join(&rel).is_file() {
            continue;
        }
        let subject_id: i64 = row.get("subject_id");
        let status: Option<String> = row.get("parse_status");
        let embed_status: Option<String> = row.get("embed_status");
        let pages: i64 = row.get("pages");
        let pdf_route = pdf_route_files(data_dir, &rel);
        let settled = match status.as_deref() {
            Some("quality") => pages > 0 && embed_status.is_none() && !needs_conversion(data_dir, &rel),
            Some("error") => !pdf_route,
            Some("skipped") => true,
            _ => false,
        };
        if settled {
            continue;
        }
        match convert(data_dir, &rel) {
            Ok(pages) => {
                record(pool, subject_id, &rel, &pages).await?;
            }
            Err(error) => {
                eprintln!("[oculus] spreadsheet {rel}: {error}");
                forget(pool, subject_id, &rel).await?;
            }
        }
        converted += 1;
    }
    Ok(converted)
}

/// App startup: [`reconcile`] on a thread of its own.
pub fn reconcile_in_background() {
    let spawned = std::thread::Builder::new().name("oculus-sheets".into()).spawn(|| {
        let data_dir = paths::data_dir();
        let outcome = tauri::async_runtime::block_on(async {
            let pool = crate::store::open_pool().await?;
            let result = reconcile(&pool, &data_dir).await;
            pool.close().await;
            result
        });
        match outcome {
            Ok(0) => {}
            Ok(n) => eprintln!("[oculus] spreadsheets: converted {n} to text"),
            Err(e) => eprintln!("[oculus] spreadsheets: {e}"),
        }
    });
    if let Err(e) = spawned {
        eprintln!("[oculus] spreadsheets: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calamine::{CellErrorType, ExcelDateTimeType};
    use std::io::Write;

    fn grid(rows: &[&[&str]]) -> Vec<Vec<String>> {
        rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect()
    }

    #[test]
    fn a_table_is_the_filled_extent_with_the_first_row_as_its_header() {
        let rows = grid(&[
            &["", "", "", ""],
            &["", "Name", "Mark", ""],
            &["", "Ada", "9", ""],
            &["", "", "", ""],
            &["", "", "", ""],
            &["", "Bo", "", ""],
            &["", "", "", ""],
        ]);
        assert_eq!(
            table(&rows).unwrap(),
            "| Name | Mark |\n| --- | --- |\n| Ada | 9 |\n|  |  |\n| Bo |  |"
        );
        assert!(table(&grid(&[&["", ""], &["", ""]])).is_none());
        assert!(table(&[]).is_none());
    }

    #[test]
    fn cells_read_as_the_sheet_shows_them() {
        assert_eq!(cell(&Data::Float(3.0)), "3");
        assert_eq!(cell(&Data::Float(-0.0)), "0");
        assert_eq!(cell(&Data::Float(0.1 + 0.2)), "0.3");
        assert_eq!(cell(&Data::Float(2.5)), "2.5");
        assert_eq!(cell(&Data::Float(1e-7)), "0.0000001");
        assert_eq!(cell(&Data::Int(-42)), "-42");
        assert_eq!(cell(&Data::Bool(true)), "TRUE");
        assert_eq!(cell(&Data::Error(CellErrorType::Div0)), "#DIV/0!");
        assert_eq!(cell(&Data::Empty), "");
        assert_eq!(cell(&Data::String("  a | b\nc\r\nd ".into())), "a \\| b<br>c<br>d");
        assert_eq!(cell(&Data::DateTimeIso("2026-03-02T09:30:00".into())), "2026-03-02T09:30:00");

        let at = |serial, kind| cell(&Data::DateTime(ExcelDateTime::new(serial, kind, false)));
        assert_eq!(at(46083.0, ExcelDateTimeType::DateTime), "2026-03-02");
        assert_eq!(at(46083.5, ExcelDateTimeType::DateTime), "2026-03-02 12:00:00");
        assert_eq!(at(0.375, ExcelDateTimeType::DateTime), "09:00:00");
        assert_eq!(at(1.5, ExcelDateTimeType::TimeDelta), "36:00:00");
    }

    /// The smallest xlsx calamine opens: two worksheets, shared strings, a
    /// number, a boolean and an empty sheet.
    fn workbook() -> Vec<u8> {
        xlsx(r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="str"><v>Pass</v></c></row>
<row r="2"><c r="A2" t="s"><v>2</v></c><c r="B2"><v>9.5</v></c><c r="C2" t="b"><v>1</v></c></row>
</sheetData></worksheet>"#)
    }

    /// An xlsx whose first sheet, "Marks", is `marks`; shared strings 0–2 are
    /// Name, Mark and Ada, and the second sheet is empty.
    fn xlsx(marks: &str) -> Vec<u8> {
        let files: &[(&str, &str)] = &[
            ("[Content_Types].xml", r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
<Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
</Types>"#),
            ("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#),
            ("xl/workbook.xml", r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets><sheet name="Marks" sheetId="1" r:id="rId1"/><sheet name="Notes" sheetId="2" r:id="rId2"/></sheets>
</workbook>"#),
            ("xl/_rels/workbook.xml.rels", r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>
</Relationships>"#),
            ("xl/sharedStrings.xml", r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="3">
<si><t>Name</t></si><si><t>Mark</t></si><si><t>Ada</t></si>
</sst>"#),
            ("xl/worksheets/sheet1.xml", marks),
            ("xl/worksheets/sheet2.xml", r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"#),
        ];
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in files {
            out.start_file(*name, options).unwrap();
            out.write_all(body.as_bytes()).unwrap();
        }
        out.finish().unwrap().into_inner()
    }

    #[test]
    fn a_workbook_becomes_one_section_per_sheet() {
        let sections = sections(&workbook()).unwrap();
        assert_eq!(
            sections,
            vec![
                "## Marks\n\n| Name | Mark | Pass |\n| --- | --- | --- |\n| Ada | 9.5 | TRUE |".to_string(),
                "## Notes\n\n(empty)".to_string(),
            ]
        );
        assert_eq!(
            document("m.xlsx", &sections),
            format!("# m.xlsx\n\n{}\n\n{}\n", sections[0], sections[1])
        );
        assert!(super::sections(b"not a workbook").is_err());
    }

    #[test]
    fn merged_blocks_fill_and_uncached_formulas_show_in_place() {
        // As a script writes it: no stored results, a shared formula copied
        // down, and "Ada" merged over two rows.
        let sheet = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="str"><v>Double</v></c></row>
<row r="2"><c r="A2" t="s"><v>2</v></c><c r="B2"><v>4</v></c><c r="C2"><f t="shared" ref="C2:C3" si="0">B2*2</f></c></row>
<row r="3"><c r="B3"><v>5</v></c><c r="C3"><f t="shared" si="0"/></c></row>
<row r="4"><c r="A4" t="str"><v>Total</v></c><c r="B4"><f>SUM(B2:B3)</f><v>9</v></c></row>
</sheetData><mergeCells count="1"><mergeCell ref="A2:A3"/></mergeCells></worksheet>"#;
        assert_eq!(
            sections(&xlsx(sheet)).unwrap()[0],
            "## Marks\n\n\
             | Name | Mark | Double |\n| --- | --- | --- |\n\
             | Ada | 4 | =B2*2 |\n| Ada | 5 | =B3*2 |\n| Total | 9 |  |\n\n\
             Formulas:\n- C2:C3: =B2*2\n- B4: =SUM(B2:B3)"
        );
    }

    #[test]
    fn a_merged_region_takes_its_top_left_value() {
        let mut range: Range<Data> = Range::new((2, 0), (5, 2));
        range.set_value((2, 0), Data::String("Content".into()));
        range.set_value((2, 1), Data::Float(1.0));
        range.set_value((3, 1), Data::Float(9.0));
        fill_merges(&mut range, &[
            Dimensions::new((2, 0), (4, 0)),
            // Runs past the range's last column: clipped, the range keeps its size.
            Dimensions::new((2, 1), (3, 9)),
            // Starts above the range, and an empty top-left: both left alone.
            Dimensions::new((0, 2), (5, 2)),
            Dimensions::new((5, 0), (5, 2)),
        ]);
        let rows: Vec<Vec<String>> = range.rows().map(|row| row.iter().map(cell).collect()).collect();
        assert_eq!(
            rows,
            grid(&[
                &["Content", "1", "1"],
                &["Content", "1", "1"],
                &["Content", "", ""],
                &["", "", ""],
            ])
        );
        assert_eq!(range.end(), Some((5, 2)));
    }

    #[test]
    fn a_formula_shows_in_place_only_where_no_result_was_stored() {
        let mut values: Range<Data> = Range::new((0, 0), (1, 0));
        values.set_value((0, 0), Data::Float(2.0));
        values.set_value((1, 0), Data::String(String::new()));
        // The formula range starts and ends elsewhere.
        let mut formulas: Range<String> = Range::new((0, 0), (2, 1));
        formulas.set_value((0, 0), "1+1".into());
        formulas.set_value((1, 0), r#"IF(1,"","x")"#.into());
        formulas.set_value((2, 1), "A1|A2".into());
        formulas.set_value((2, 0), "of:=[.A1]*2".into());
        let rows: Vec<Vec<String>> = texts(&values, &formulas).rows().map(<[String]>::to_vec).collect();
        assert_eq!(
            rows,
            grid(&[
                &["2", ""],
                &["", ""],
                &["=[.A1]*2", "=A1\\|A2"],
            ])
        );
        assert!(texts(&Range::empty(), &Range::empty()).is_empty());
    }

    #[test]
    fn references_are_made_relative_to_their_cell() {
        // G9 seen from D9, and from D10 one row down: the same pattern.
        assert_eq!(relative("PROPER(G9)", (8, 3)), "PROPER(R[0]C[3])");
        assert_eq!(relative("PROPER(G10)", (9, 3)), relative("PROPER(G9)", (8, 3)));
        assert_eq!(relative("$G$9+G$9+$G9", (8, 3)), "R9C7+R9C[3]+R[0]C7");
        assert_eq!(relative("SUM(B2:B10)", (10, 1)), "SUM(R[-9]C[0]:R[-1]C[0])");
        assert_eq!(relative("SUM($B$2:$B$10)", (10, 1)), "SUM(R2C2:R10C2)");
        assert_eq!(
            relative("Marks!B2+'My Sheet'!$C$3+'Q1''s A1'!A1", (0, 0)),
            "Marks!R[1]C[1]+'My Sheet'!R3C3+'Q1''s A1'!R[0]C[0]"
        );
        assert_eq!(
            relative(r#"IF(A1="B2","say ""C3"" ok",LOG10(A1))"#, (0, 0)),
            r#"IF(R[0]C[0]="B2","say ""C3"" ok",LOG10(R[0]C[0]))"#
        );
        // ods writes `[.A1]` and `[Sheet2.A1]`.
        assert_eq!(relative("SUM([.A1:.A3])", (3, 0)), "SUM([.R[-3]C[0]:.R[-1]C[0]])");
        // Numbers, names, columns past XFD, row 0 and lower case are not references.
        let plain = "1E5+TAX2020X+XFE1+A0+a1+_xlfn.CONCAT(Ü1)";
        assert_eq!(relative(plain, (0, 0)), plain);
        assert_eq!(relative("XFD1048576", (0, 0)), "R[1048575]C[16383]");
    }

    #[test]
    fn cells_are_named_in_a1() {
        let names: Vec<String> = [0, 25, 26, 701, 702, 16_383].into_iter().map(column_name).collect();
        assert_eq!(names, ["A", "Z", "AA", "ZZ", "AAA", "XFD"]);
        assert_eq!(block_name((8, 3), (8, 3)), "D9");
        assert_eq!(block_name((8, 3), (13, 4)), "D9:E14");
    }

    #[test]
    fn runs_collapse_into_blocks() {
        let rect: Vec<(u32, u32)> = (0..3).flat_map(|r| (0..2).map(move |c| (r, c))).collect();
        assert_eq!(blocks(&rect), vec![((0, 0), (2, 1))]);
        // A row run with one cell under its start: the longer run wins.
        assert_eq!(blocks(&[(0, 0), (0, 1), (0, 2), (1, 0)]), vec![((0, 0), (0, 2)), ((1, 0), (1, 0))]);
        assert_eq!(blocks(&[(0, 0), (2, 0), (3, 0)]), vec![((0, 0), (0, 0)), ((2, 0), (3, 0))]);
    }

    #[test]
    fn a_sheet_lists_its_formulas_once_per_pattern() {
        let mut formulas: Range<String> = Range::new((0, 0), (19, 5));
        formulas.set_value((0, 0), "SUM($B$1:$B$3)".into());
        for row in 8..14 {
            // D and F each copied down, both reading three columns right.
            formulas.set_value((row, 3), format!("PROPER(G{})", row + 1));
            formulas.set_value((row, 5), format!("PROPER(I{})", row + 1));
        }
        for col in 1..4 {
            // Copied across row 20.
            formulas.set_value((19, col), format!("{0}19*2", column_name(col)));
        }
        assert_eq!(
            formula_list(&formulas).unwrap(),
            "- A1: =SUM($B$1:$B$3)\n- D9:D14, F9:F14: =PROPER(G9)\n- B20:D20: =B19*2"
        );
        assert_eq!(formula_list(&Range::new((0, 0), (1, 1))), None);

        let mut many: Range<String> = Range::new((0, 0), (204, 0));
        for row in 0..205 {
            many.set_value((row, 0), format!("{row}+1"));
        }
        let list = formula_list(&many).unwrap();
        let lines: Vec<&str> = list.lines().collect();
        assert_eq!((lines.len(), lines[0], lines[200]), (201, "- A1: =0+1", "- … and 5 more"));
    }

    #[test]
    fn converting_writes_the_text_and_clears_the_pdf_route() {
        let scratch = crate::test_support::Scratch::new("sheets-convert");
        let dir = scratch.join("courses/X/files");
        std::fs::create_dir_all(dir.join("m.xlsx_images")).unwrap();
        std::fs::write(dir.join("m.xlsx"), workbook()).unwrap();
        for stale in ["m.xlsx.pdf", "m.xlsx.pages.json", "m.xlsx.md"] {
            std::fs::write(dir.join(stale), b"old").unwrap();
        }
        let rel = "courses/X/files/m.xlsx";
        assert!(needs_conversion(&scratch, rel));

        let pages = convert(&scratch, rel).unwrap();
        assert_eq!(pages.iter().map(|p| p.page_no).collect::<Vec<_>>(), vec![1, 2]);
        assert!(pages[0].markdown.starts_with("## Marks"));
        let md = std::fs::read_to_string(dir.join("m.xlsx.md")).unwrap();
        assert!(md.starts_with("# m.xlsx\n\n## Marks"), "{md}");
        assert!(!needs_conversion(&scratch, rel));
        assert!(!dir.join("m.xlsx.pdf").exists() && !dir.join("m.xlsx_images").exists());

        // Bytes that no longer read leave no text behind.
        std::fs::write(dir.join("m.xlsx"), b"garbage").unwrap();
        let error = convert(&scratch, rel).unwrap_err();
        assert_eq!(error.kind(), "document");
        assert!(!dir.join("m.xlsx.md").exists());
    }

    #[tokio::test]
    async fn recording_replaces_the_pages_and_marks_the_sheet_finished() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE files (id INTEGER PRIMARY KEY AUTOINCREMENT, subject_id INTEGER NOT NULL,
               filename TEXT NOT NULL, relative_path TEXT NOT NULL, file_type TEXT NOT NULL,
               first_seen_at TEXT, parse_status TEXT, parsed_at TEXT, embed_status TEXT,
               embedded_at TEXT, UNIQUE(subject_id, relative_path));
             CREATE TABLE pages (id INTEGER PRIMARY KEY AUTOINCREMENT, file_id INTEGER NOT NULL,
               page_no INTEGER NOT NULL, markdown TEXT NOT NULL DEFAULT '', embedding BLOB,
               embed_model TEXT, embed_dim INTEGER, embedded_at TEXT, UNIQUE(file_id, page_no));",
        )
        .execute(&pool)
        .await
        .unwrap();

        let rel = "courses/X/files/m.xlsx";
        let page = |n: u32, text: &str| ParsePage { page_no: n, markdown: text.into() };
        // No row yet: the conversion beat the frontend's upsert.
        record(&pool, 7, rel, &[page(1, "a"), page(2, "b"), page(3, "c")]).await.unwrap();
        sqlx::query("UPDATE pages SET embedding = x'00', embed_model = 'm'").execute(&pool).await.unwrap();
        sqlx::query("UPDATE files SET embed_status = 'error'").execute(&pool).await.unwrap();

        assert_eq!(record(&pool, 7, rel, &[page(1, "a2")]).await.unwrap(), 1);
        let pages: Vec<(i64, String, Option<Vec<u8>>)> =
            sqlx::query_as("SELECT page_no, markdown, embedding FROM pages ORDER BY page_no")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(pages, vec![(1, "a2".to_string(), None)]);
        let (filename, file_type, status, embed): (String, String, String, Option<String>) =
            sqlx::query_as("SELECT filename, file_type, parse_status, embed_status FROM files")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((filename.as_str(), file_type.as_str(), status.as_str()), ("m.xlsx", "xlsx", "quality"));
        assert_eq!(embed, None);

        forget(&pool, 7, rel).await.unwrap();
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages").fetch_one(&pool).await.unwrap();
        let status: String = sqlx::query_scalar("SELECT parse_status FROM files").fetch_one(&pool).await.unwrap();
        assert_eq!((left, status.as_str()), (0, "error"));
    }
}

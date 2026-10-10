//! A workbook read with calamine into one markdown section per worksheet.

use std::io::{Cursor, Read, Seek};

use calamine::{CellType, Data, Dimensions, Range, Reader, SheetType, Sheets};

use super::csv::{cell, escape};
use super::formulas::{formula_body, formula_list};

/// One `## sheet` section per worksheet, in workbook order. Chart, dialog and
/// macro sheets hold no cells and are left out.
pub fn sections(bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut book =
        calamine::open_workbook_auto_from_rs(Cursor::new(bytes)).map_err(|e| e.to_string())?;
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
            let values = book
                .worksheet_range(name)
                .map_err(|e| failed(e.to_string()))?;
            let formulas = book
                .worksheet_formula(name)
                .map_err(|e| failed(e.to_string()))?;
            let merges = merged_regions(&mut book, name).map_err(failed)?;
            Ok(section(name, &values, &formulas, &merges))
        })
        .collect()
}

/// A sheet's merged regions in absolute sheet coordinates. xlsx and xls
/// record them; calamine reads none from ods or xlsb, so there a block keeps
/// its value in its top-left cell only.
fn merged_regions<RS: Read + Seek>(
    book: &mut Sheets<RS>,
    name: &str,
) -> Result<Vec<Dimensions>, String> {
    match book {
        Sheets::Xlsx(xlsx) => xlsx
            .merge_cells_by_sheet_name(name)
            .map_err(|e| e.to_string()),
        Sheets::Xls(xls) => xls
            .merge_cells_by_sheet_name(name)
            .map_err(|e| e.to_string()),
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
fn section(
    name: &str,
    values: &Range<Data>,
    formulas: &Range<String>,
    merges: &[Dimensions],
) -> String {
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
pub(super) fn texts(values: &Range<Data>, formulas: &Range<String>) -> Range<String> {
    let mut extents = [
        values.start().zip(values.end()),
        formulas.start().zip(formulas.end()),
    ]
    .into_iter()
    .flatten();
    let Some(first) = extents.next() else {
        return Range::empty();
    };
    let (start, end) = extents.fold(first, |(s, e), (s2, e2)| {
        (
            (s.0.min(s2.0), s.1.min(s2.1)),
            (e.0.max(e2.0), e.1.max(e2.1)),
        )
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
pub(super) fn fill_merges<T: CellType>(range: &mut Range<T>, merges: &[Dimensions]) {
    let Some(end) = range.end() else { return };
    for region in merges {
        let Some(value) = range
            .get_value(region.start)
            .filter(|v| **v != T::default())
            .cloned()
        else {
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
pub(super) fn table(rows: &[Vec<String>]) -> Option<String> {
    let filled = |row: &Vec<String>| row.iter().any(|c| !c.is_empty());
    let first = rows.iter().position(filled)?;
    let last = rows.iter().rposition(filled)?;
    let rows: Vec<&Vec<String>> = rows[first..=last]
        .iter()
        .enumerate()
        .filter(|&(i, row)| filled(row) || filled(&rows[first + i - 1]))
        .map(|(_, row)| row)
        .collect();
    let column_filled = |i: usize| {
        rows.iter()
            .any(|row| row.get(i).is_some_and(|c| !c.is_empty()))
    };
    let width = rows.iter().map(|row| row.len()).max().unwrap_or(0);
    let left = (0..width).find(|&i| column_filled(i))?;
    let right = (0..width).rev().find(|&i| column_filled(i))?;

    let line = |row: &&Vec<String>| {
        let cells: Vec<&str> = (left..=right)
            .map(|i| row.get(i).map(String::as_str).unwrap_or(""))
            .collect();
        format!("| {} |", cells.join(" | "))
    };
    let mut out = vec![line(&rows[0])];
    out.push(format!("|{}", " --- |".repeat(right - left + 1)));
    out.extend(rows[1..].iter().map(line));
    Some(out.join("\n"))
}

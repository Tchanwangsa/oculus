//! Spreadsheets as text. A workbook (xlsx, xlsm, xls, ods) is read in-process
//! with calamine, a CSV by `csv_rows`, and written as `{name}.md` beside it: `# {name}`, then one
//! `##` section per worksheet holding its used range as a GFM table of values
//! (merged blocks filled, `=FORMULA` where no result was stored), then its
//! formulas, one line per copied-down or -across pattern. Each sheet is one
//! `pages` row (page 1 is the first sheet), so `oculus read`, `oculus grep`
//! and the lexical index see it. Nothing is sent to a parse service and
//! nothing is embedded: a table has no page image worth ranking.
//! See `docs/parsing.md`.

mod convert;
mod csv;
mod db;
mod formulas;
#[cfg(test)]
mod tests;
mod workbook;

pub use convert::convert;
pub use db::{index, reconcile, reconcile_in_background, record};
pub use workbook::{document, sections};

use std::path::Path;

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

use std::io::Cursor;

use calamine::{Data, Dimensions, ExcelDateTime, Range};

use crate::parse::ParsePage;

use super::csv::{cell, csv_rows, csv_section, delimiter};
use super::db::forget;
use super::formulas::{block_name, blocks, column_name, formula_list, relative};
use super::workbook::{fill_merges, table, texts};
use super::*;

#[test]
fn a_csv_reads_quotes_line_breaks_and_its_locale_delimiter() {
    let rows = csv_rows("a,\"b, \"\"c\"\"\"\r\n1,\"two\nlines\"\n3", ',');
    assert_eq!(
        rows,
        vec![
            vec!["a".to_string(), "b, \"c\"".into()],
            vec!["1".into(), "two\nlines".into()],
            vec!["3".into()],
        ]
    );
    assert_eq!(delimiter("name;mark;note, if any\n"), ';');
    assert_eq!(delimiter("a\tb\tc\n"), '\t');
    assert_eq!(delimiter("a|b|c\n"), '|');
    assert_eq!(delimiter("single\n"), ',');
}

#[test]
fn a_csv_is_one_section_named_after_its_file() {
    let bytes = b"\xEF\xBB\xBFName,Mark\nAda,9\nBo|b,\n";
    assert_eq!(
        csv_section("marks.csv", bytes),
        "## marks.csv\n\n| Name | Mark |\n| --- | --- |\n| Ada | 9 |\n| Bo\\|b |  |"
    );
    // Not UTF-8: read as Latin-1, never refused.
    assert!(csv_section("x.csv", b"caf\xE9\n").contains("caf\u{e9}"));
    assert!(csv_section("empty.csv", b"").ends_with("(empty)"));
}
use calamine::{CellErrorType, ExcelDateTimeType};
use std::io::Write;

fn grid(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect()
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
    assert_eq!(
        cell(&Data::String("  a | b\nc\r\nd ".into())),
        "a \\| b<br>c<br>d"
    );
    assert_eq!(
        cell(&Data::DateTimeIso("2026-03-02T09:30:00".into())),
        "2026-03-02T09:30:00"
    );

    let at = |serial, kind| cell(&Data::DateTime(ExcelDateTime::new(serial, kind, false)));
    assert_eq!(at(46083.0, ExcelDateTimeType::DateTime), "2026-03-02");
    assert_eq!(
        at(46083.5, ExcelDateTimeType::DateTime),
        "2026-03-02 12:00:00"
    );
    assert_eq!(at(0.375, ExcelDateTimeType::DateTime), "09:00:00");
    assert_eq!(at(1.5, ExcelDateTimeType::TimeDelta), "36:00:00");
}

/// The smallest xlsx calamine opens: two worksheets, shared strings, a
/// number, a boolean and an empty sheet.
fn workbook() -> Vec<u8> {
    xlsx(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="str"><v>Pass</v></c></row>
<row r="2"><c r="A2" t="s"><v>2</v></c><c r="B2"><v>9.5</v></c><c r="C2" t="b"><v>1</v></c></row>
</sheetData></worksheet>"#,
    )
}

/// An xlsx whose first sheet, "Marks", is `marks`; shared strings 0–2 are
/// Name, Mark and Ada, and the second sheet is empty.
fn xlsx(marks: &str) -> Vec<u8> {
    let files: &[(&str, &str)] = &[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
<Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
</Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
        ),
        (
            "xl/workbook.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets><sheet name="Marks" sheetId="1" r:id="rId1"/><sheet name="Notes" sheetId="2" r:id="rId2"/></sheets>
</workbook>"#,
        ),
        (
            "xl/_rels/workbook.xml.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>
</Relationships>"#,
        ),
        (
            "xl/sharedStrings.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="3">
<si><t>Name</t></si><si><t>Mark</t></si><si><t>Ada</t></si>
</sst>"#,
        ),
        ("xl/worksheets/sheet1.xml", marks),
        (
            "xl/worksheets/sheet2.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"#,
        ),
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
            "## Marks\n\n| Name | Mark | Pass |\n| --- | --- | --- |\n| Ada | 9.5 | TRUE |"
                .to_string(),
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
    fill_merges(
        &mut range,
        &[
            Dimensions::new((2, 0), (4, 0)),
            // Runs past the range's last column: clipped, the range keeps its size.
            Dimensions::new((2, 1), (3, 9)),
            // Starts above the range, and an empty top-left: both left alone.
            Dimensions::new((0, 2), (5, 2)),
            Dimensions::new((5, 0), (5, 2)),
        ],
    );
    let rows: Vec<Vec<String>> = range
        .rows()
        .map(|row| row.iter().map(cell).collect())
        .collect();
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
    let rows: Vec<Vec<String>> = texts(&values, &formulas)
        .rows()
        .map(<[String]>::to_vec)
        .collect();
    assert_eq!(
        rows,
        grid(&[&["2", ""], &["", ""], &["=[.A1]*2", "=A1\\|A2"],])
    );
    assert!(texts(&Range::empty(), &Range::empty()).is_empty());
}

#[test]
fn references_are_made_relative_to_their_cell() {
    // G9 seen from D9, and from D10 one row down: the same pattern.
    assert_eq!(relative("PROPER(G9)", (8, 3)), "PROPER(R[0]C[3])");
    assert_eq!(
        relative("PROPER(G10)", (9, 3)),
        relative("PROPER(G9)", (8, 3))
    );
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
    assert_eq!(
        relative("SUM([.A1:.A3])", (3, 0)),
        "SUM([.R[-3]C[0]:.R[-1]C[0]])"
    );
    // Numbers, names, columns past XFD, row 0 and lower case are not references.
    let plain = "1E5+TAX2020X+XFE1+A0+a1+_xlfn.CONCAT(Ü1)";
    assert_eq!(relative(plain, (0, 0)), plain);
    assert_eq!(relative("XFD1048576", (0, 0)), "R[1048575]C[16383]");
}

#[test]
fn cells_are_named_in_a1() {
    let names: Vec<String> = [0, 25, 26, 701, 702, 16_383]
        .into_iter()
        .map(column_name)
        .collect();
    assert_eq!(names, ["A", "Z", "AA", "ZZ", "AAA", "XFD"]);
    assert_eq!(block_name((8, 3), (8, 3)), "D9");
    assert_eq!(block_name((8, 3), (13, 4)), "D9:E14");
}

#[test]
fn runs_collapse_into_blocks() {
    let rect: Vec<(u32, u32)> = (0..3).flat_map(|r| (0..2).map(move |c| (r, c))).collect();
    assert_eq!(blocks(&rect), vec![((0, 0), (2, 1))]);
    // A row run with one cell under its start: the longer run wins.
    assert_eq!(
        blocks(&[(0, 0), (0, 1), (0, 2), (1, 0)]),
        vec![((0, 0), (0, 2)), ((1, 0), (1, 0))]
    );
    assert_eq!(
        blocks(&[(0, 0), (2, 0), (3, 0)]),
        vec![((0, 0), (0, 0)), ((2, 0), (3, 0))]
    );
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
    assert_eq!(
        (lines.len(), lines[0], lines[200]),
        (201, "- A1: =0+1", "- … and 5 more")
    );
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
    assert_eq!(
        pages.iter().map(|p| p.page_no).collect::<Vec<_>>(),
        vec![1, 2]
    );
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
    let page = |n: u32, text: &str| ParsePage {
        page_no: n,
        markdown: text.into(),
        blocks: Vec::new(),
    };
    // No row yet: the conversion beat the frontend's upsert.
    record(&pool, 7, rel, &[page(1, "a"), page(2, "b"), page(3, "c")])
        .await
        .unwrap();
    sqlx::query("UPDATE pages SET embedding = x'00', embed_model = 'm'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE files SET embed_status = 'error'")
        .execute(&pool)
        .await
        .unwrap();

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
    assert_eq!(
        (filename.as_str(), file_type.as_str(), status.as_str()),
        ("m.xlsx", "xlsx", "quality")
    );
    assert_eq!(embed, None);

    forget(&pool, 7, rel).await.unwrap();
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages")
        .fetch_one(&pool)
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT parse_status FROM files")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((left, status.as_str()), (0, "error"));
}

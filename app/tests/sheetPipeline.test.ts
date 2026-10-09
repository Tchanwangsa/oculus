import { describe, expect, test } from "bun:test";
import {
  PDF_BACKED_SQL_LIST,
  PIPELINE_SQL_LIST,
  isPdfBacked,
  isPipelineFile,
  isSheetFile,
  parsedMdRelPath,
  parsedMdSource,
} from "../src/lib/fileTypes";
import { hasFailed, isComplete, statusOf, type PipelineItem } from "../src/stores/pipelineStore";

const row = (filename: string, patch: Partial<PipelineItem>): PipelineItem => ({
  relativePath: `courses/X/files/${filename}`,
  subjectId: 1,
  code: "X",
  filename,
  download: "done",
  parse: "done",
  embed: "pending",
  pagesDone: 0,
  totalPages: 0,
  embedPagesDone: 0,
  embedTotalPages: 0,
  paused: false,
  startedAt: 0,
  updatedAt: 0,
  ...patch,
});

describe("spreadsheets", () => {
  test("are pipeline files with text beside them, never PDF-backed", () => {
    for (const name of ["marks.xlsx", "MACROS.XLSM", "old.xls", "calc.ods", "grades.CSV"]) {
      expect(isSheetFile(name)).toBe(true);
      expect(isPdfBacked(name)).toBe(false);
      expect(isPipelineFile(name)).toBe(true);
    }
    const file = { filename: "marks.xlsx", relative_path: "courses/X/files/marks.xlsx" };
    expect(parsedMdRelPath(file)).toBe("courses/X/files/marks.xlsx.md");
    expect(parsedMdSource("courses/X/files/marks.xlsx.md")).toBe("courses/X/files/marks.xlsx");
  });

  test("the SQL lists mirror paths.rs", () => {
    // `pdf_backed_sql_list` in paths.rs asserts the same string.
    expect(PDF_BACKED_SQL_LIST).toBe("('pdf', 'pptx', 'docx', 'ppt', 'doc')");
    expect(PIPELINE_SQL_LIST).toBe("('pdf', 'pptx', 'docx', 'ppt', 'doc', 'xlsx', 'xlsm', 'xls', 'ods', 'csv')");
  });

  test("a sheet's row is done at its conversion, even with the embed stage on", () => {
    const sheet = row("marks.xlsx", {});
    expect(isComplete(sheet, true)).toBe(true);
    expect(statusOf(sheet, true).label).toBe("Converted to text");
    // An old embed failure on a sheet is not drawn.
    expect(hasFailed(row("marks.xlsx", { embed: "error" }), true)).toBe(false);

    const pdf = row("w1.pdf", {});
    expect(isComplete(pdf, true)).toBe(false);
    expect(statusOf(pdf, false).label).toBe("Parsed");
  });
});

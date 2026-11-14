import { describe, expect, test } from "bun:test";
import { createSavedRecorder } from "@/lib/browser/downloads/recordSaved";
import type { Download } from "@/lib/browser/downloads/types";
import type { ImportOutcome } from "@/lib/files/uploads";
import { download } from "./fixtures";

const SUBJECT = { id: 42, code: "COMP10001_2026_SM2" };

function outcome(filename: string, error: string | null = null): ImportOutcome {
  return {
    source: filename,
    file: { filename, relative_path: `courses/${SUBJECT.code}/uploads/${filename}`, file_type: "x", size_bytes: 10 },
    error,
  };
}

function saved(id: number, o: ImportOutcome): Download {
  return download(id, { state: "saved", saved_to: { kind: "oculus", subject_code: SUBJECT.code, outcome: o } });
}

/** A recorder over an in-memory files table, logging what it writes. */
function harness(existing: string[] = []) {
  const rows = new Set(existing);
  const written: string[] = [];
  let failNext = false;
  const record = createSavedRecorder({
    subjectOf: async (code) => (code === SUBJECT.code ? SUBJECT : null),
    hasRow: async (path) => rows.has(path),
    record: async (subject, outcomes) => {
      expect(subject).toEqual(SUBJECT);
      if (failNext) {
        failNext = false;
        throw new Error("database is locked");
      }
      for (const o of outcomes) {
        rows.add(o.file!.relative_path);
        written.push(o.file!.filename);
      }
    },
  });
  return { record, written, failOnce: () => { failNext = true; } };
}

describe("recording downloads saved in Oculus", () => {
  test("each saved download is recorded once, however often the list repeats", async () => {
    const { record, written } = harness();
    const list = [saved(1, outcome("a.pdf"))];
    await Promise.all([record(list), record(list)]);
    await record(list);
    expect(written).toEqual(["a.pdf"]);
  });

  test("a list replayed after a reload records nothing already in the database", async () => {
    const { record, written } = harness([`courses/${SUBJECT.code}/uploads/a.pdf`]);
    await record([saved(1, outcome("a.pdf")), saved(2, outcome("b.pdf"))]);
    expect(written).toEqual(["b.pdf"]);
  });

  test("a file whose conversion failed is still recorded; the parse is left to recordUploads", async () => {
    const { record, written } = harness();
    await record([saved(1, outcome("notes.docx", "added, but not converted for search"))]);
    expect(written).toEqual(["notes.docx"]);
  });

  test("rows that are not saved in Oculus are left alone", async () => {
    const { record, written } = harness();
    await record([
      download(1),
      download(2, { state: "ready" }),
      download(3, { state: "saved", saved_to: { kind: "path", path: "/Users/s/Downloads/c.zip" } }),
      download(4, { state: "failed", error: "x" }),
    ]);
    expect(written).toEqual([]);
  });

  test("a failed write is tried again on the next list", async () => {
    const { record, written, failOnce } = harness();
    const list = [saved(1, outcome("a.pdf"))];
    failOnce();
    await record(list);
    expect(written).toEqual([]);
    await record(list);
    expect(written).toEqual(["a.pdf"]);
  });
});

import { afterAll, expect, spyOn, test } from "bun:test";
import { Database as SQLite } from "bun:sqlite";
import Database from "@tauri-apps/plugin-sql";
import { searchLibraryFiles } from "@/lib/db/search";
import { getDb, getSubjects, setParseStatus } from "@/lib/db";

const sqlite = new SQLite(":memory:");
const fakeDb = {
  select: async (sql: string, bindings: Array<string | number> = []) => sqlite.query(sql).all(...bindings),
  execute: async (sql: string, bindings: Array<string | number> = []) => {
    const result = sqlite.query(sql).run(...bindings);
    return { rowsAffected: result.changes };
  },
} as unknown as Database;
afterAll(() => sqlite.close());

test("concurrent database users share initialization and recover from a failed load", async () => {
  let rejectLoad!: (error: Error) => void;
  const load = spyOn(Database, "load").mockImplementationOnce(() => new Promise((_, reject) => {
    rejectLoad = reject;
  })).mockResolvedValue(fakeDb);
  try {
    const first = getDb();
    const second = getDb();
    expect(first).toBe(second);
    expect(load).toHaveBeenCalledTimes(1);
    const rejected = Promise.allSettled([first, second]);
    rejectLoad(new Error("unavailable"));
    expect((await rejected).every((result) => result.status === "rejected")).toBe(true);
    const retry = getDb();
    expect(getDb()).toBe(retry);
    expect(await retry).toBe(fakeDb);
    expect(await getDb()).toBe(fakeDb);
    expect(load).toHaveBeenCalledTimes(2);
  } finally {
    load.mockRestore();
  }
});

test("subject sync aggregation preserves latest completed runs, term ordering and selection", async () => {
  sqlite.exec(`CREATE TABLE subjects (
    id INTEGER PRIMARY KEY, code TEXT, name TEXT, term_name TEXT, term_sort TEXT,
    workflow_state TEXT, selected INTEGER, is_current INTEGER,
    last_synced_at TEXT, created_at TEXT
  ); CREATE TABLE sync_runs (status TEXT, finished_at TEXT, subject_codes TEXT);
  INSERT INTO subjects VALUES
    (1, 'A', 'Alpha', '2026 Semester 2', '2026-07', 'available', 0, 0, 'stale', ''),
    (2, 'B', 'Beta', '2026 Summer Term', '2026-01', 'available', 1, 1, 'stale', ''),
    (3, 'C', 'Closed', '2027 Semester 1', '2027-03', 'completed', 1, 1, NULL, ''),
    (4, 'D', 'Delta', '2026 Semester 2', '2026-07', 'available', 1, 0, NULL, ''),
    (5, 'E', 'Empty', NULL, NULL, 'available', 0, 0, NULL, '');
  INSERT INTO sync_runs VALUES
    ('completed', '2026-01-01', '["A", "B"]'),
    ('completed', '2026-02-01', '["A", "A"]'),
    ('failed', '2026-03-01', '["A", "D"]'),
    ('running', '2026-04-01', '["D"]'),
    ('completed', '2026-05-01', NULL),
    ('completed', '2026-06-01', '[]');`);
  const rows = await getSubjects();
  expect(rows.map((row) => row.id)).toEqual([1, 4, 3, 2, 5]);
  expect(rows.map((row) => row.last_synced_at)).toEqual([
    "2026-02-01", null, null, "2026-01-01", null,
  ]);
  expect(rows.map((row) => row.is_current)).toEqual([true, true, false, false, false]);
  expect(rows.map((row) => row.selected)).toEqual([false, true, true, true, false]);
});


test("parse persistence reports missing rows and stamps successful parses", async () => {
  sqlite.exec(`CREATE TABLE files (
    subject_id INTEGER, relative_path TEXT, parse_status TEXT, parsed_at TEXT
  ); INSERT INTO files VALUES (1, 'a.pdf', NULL, NULL);`);
  expect(await setParseStatus(1, "missing.pdf", "running")).toBe(false);
  expect(await setParseStatus(1, "a.pdf", "running")).toBe(true);
  expect(sqlite.query("SELECT parse_status, parsed_at FROM files").get())
    .toEqual({ parse_status: "running", parsed_at: null });
  expect(await setParseStatus(1, "a.pdf", "quality")).toBe(true);
  expect(sqlite.query("SELECT parse_status, parsed_at FROM files").get())
    .toMatchObject({ parse_status: "quality", parsed_at: expect.any(String) });
});


test("library PDF search filters before the limit and matches filename plus subject", async () => {
  sqlite.exec(`DROP TABLE files;
    CREATE TABLE files (id INTEGER PRIMARY KEY, subject_id INTEGER, filename TEXT,
      relative_path TEXT, file_type TEXT, category TEXT, last_accessed_at TEXT);
    INSERT INTO files VALUES
      (1, 2, 'algebra-notes.md', 'courses/B/notes.md', 'md', 'document', '2026-10-20'),
      (2, 2, 'algebra-workshop.pdf', 'courses/B/workshop.pdf', 'pdf', 'file', '2026-10-19'),
      (3, 2, 'algebra-lecture.pdf', 'courses/B/lecture.pdf', 'pdf', 'upload', '2026-10-18'),
      (4, 1, 'geometry.pdf', 'courses/A/geometry.pdf', 'pdf', 'file', NULL);`);
  const pdfs = (query: string, limit = 30) => searchLibraryFiles(query, limit, { fileType: 'pdf' });
  expect((await pdfs('', 1)).map(file => file.id)).toEqual([2]);
  expect((await pdfs('B algebra')).map(file => file.id)).toEqual([2, 3]);
  expect((await pdfs('workshop B')).map(file => file.id)).toEqual([2]);
  expect(await pdfs('notes')).toEqual([]);
  expect((await searchLibraryFiles('notes')).map(file => file.id)).toEqual([1]);
  expect(await pdfs('%')).toEqual([]);
  expect((await pdfs('geometry'))[0]).toMatchObject({ subject_code: 'A', relative_path: 'courses/A/geometry.pdf' });
});

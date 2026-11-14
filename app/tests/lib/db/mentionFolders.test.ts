import { afterAll, expect, test } from "bun:test";
import { Database as SQLite } from "bun:sqlite";
import { mentionFoldersQuery, type MentionFolder } from "@/lib/db/mentions";

const sqlite = new SQLite(":memory:");
sqlite.exec(`CREATE TABLE subjects (id INTEGER PRIMARY KEY, code TEXT);
  CREATE TABLE files (id INTEGER PRIMARY KEY, subject_id INTEGER, relative_path TEXT, last_accessed_at TEXT);
  INSERT INTO subjects VALUES (1, 'MAST20004'), (2, 'COMP10001');
  INSERT INTO files VALUES
    (1, 1, 'courses/MAST20004/pages/week-1.md', '2026-11-01'),
    (2, 1, 'courses/MAST20004/files/Week_1/notes.pdf', '2026-11-10'),
    (3, 1, 'courses/MAST20004/files/Week_2/notes.pdf', NULL),
    (4, 1, 'courses/MAST20004/home.md', '2026-11-12'),
    (5, 1, 'courses/MAST20004/uploads/mine.pdf', NULL),
    (6, 2, 'courses/COMP10001/pages/intro.md', '2026-11-05');`);
afterAll(() => sqlite.close());

// The statement `searchMentionFolders` runs, against an in-memory SQLite.
function folders(subjectId: number | null, query: string, limit = 6): MentionFolder[] {
  const { sql, params } = mentionFoldersQuery(subjectId, query, limit);
  return sqlite.query(sql).all(...params) as MentionFolder[];
}

test("a subject's bare @ lists its top-level folders, most recently opened first", () => {
  expect(folders(1, "").map((f) => [f.path, f.name])).toEqual([
    ["courses/MAST20004/files/", "files"],
    ["courses/MAST20004/pages/", "pages"],
    ["courses/MAST20004/uploads/", "uploads"],
  ]);
});

test("a query narrows the folders by name", () => {
  expect(folders(1, "up").map((f) => f.path)).toEqual(["courses/MAST20004/uploads/"]);
  expect(folders(1, "nothing")).toEqual([]);
});

test("a General thread's folders are the subjects'", () => {
  expect(folders(null, "").map((f) => [f.path, f.name, f.subject_code])).toEqual([
    ["courses/MAST20004/", "MAST20004", "MAST20004"],
    ["courses/COMP10001/", "COMP10001", "COMP10001"],
  ]);
});

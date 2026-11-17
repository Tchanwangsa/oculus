import { afterAll, expect, test } from "bun:test";
import { Database as SQLite } from "bun:sqlite";
import { mentionFoldersQuery, type MentionFolder } from "@/lib/db/mentions";

const sqlite = new SQLite(":memory:");
sqlite.exec(`CREATE TABLE subjects (id INTEGER PRIMARY KEY, code TEXT);
  CREATE TABLE files (id INTEGER PRIMARY KEY, subject_id INTEGER, relative_path TEXT, last_accessed_at TEXT);
  INSERT INTO subjects VALUES (1, 'MAST20004'), (2, 'COMP10001');
  INSERT INTO files VALUES
    (1, 1, 'courses/MAST20004/Course/Pages/week-1.md', '2026-11-01'),
    (2, 1, 'courses/MAST20004/Course/Files/Week_1/notes.pdf', '2026-11-10'),
    (3, 1, 'courses/MAST20004/Course/Files/Week_2/notes.pdf', NULL),
    (4, 1, 'courses/MAST20004/Course/home.md', '2026-11-12'),
    (5, 1, 'courses/MAST20004/Exam prep/mine.pdf', NULL),
    (7, 1, 'courses/MAST20004/loose.pdf', NULL),
    (6, 2, 'courses/COMP10001/Course/Pages/intro.md', '2026-11-05');`);
afterAll(() => sqlite.close());

// The statement `searchMentionFolders` runs, against an in-memory SQLite.
function folders(subjectId: number | null, query: string, limit = 6): MentionFolder[] {
  const { sql, params } = mentionFoldersQuery(subjectId, query, limit);
  return sqlite.query(sql).all(...params) as MentionFolder[];
}

test("a subject's bare @ lists its top-level folders, Course's opened, most recently opened first", () => {
  expect(folders(1, "").map((f) => [f.path, f.name])).toEqual([
    ["courses/MAST20004/Course/Files/", "Files"],
    ["courses/MAST20004/Course/Pages/", "Pages"],
    ["courses/MAST20004/Exam prep/", "Exam prep"],
  ]);
});

test("a query narrows the folders by name", () => {
  expect(folders(1, "exam").map((f) => f.path)).toEqual(["courses/MAST20004/Exam prep/"]);
  expect(folders(1, "nothing")).toEqual([]);
});

test("a General thread's folders are the subjects'", () => {
  expect(folders(null, "").map((f) => [f.path, f.name, f.subject_code])).toEqual([
    ["courses/MAST20004/", "MAST20004", "MAST20004"],
    ["courses/COMP10001/", "COMP10001", "COMP10001"],
  ]);
});

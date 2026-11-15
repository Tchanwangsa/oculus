import { afterAll, expect, test } from "bun:test";
import { Database as SQLite } from "bun:sqlite";
import { mentionFolderQueries, type MentionFile, type MentionFolder } from "@/lib/db/mentions";

const sqlite = new SQLite(":memory:");
sqlite.exec(`CREATE TABLE subjects (id INTEGER PRIMARY KEY, code TEXT);
  CREATE TABLE files (id INTEGER PRIMARY KEY, subject_id INTEGER, relative_path TEXT, filename TEXT,
    category TEXT, file_type TEXT, parse_status TEXT);
  INSERT INTO subjects VALUES (1, 'MAST20004'), (2, 'MAST2000X');
  INSERT INTO files VALUES
    (1, 1, 'courses/MAST20004/home.md', 'home.md', 'page', 'md', NULL),
    (2, 1, 'courses/MAST20004/files/syllabus.pdf', 'syllabus.pdf', 'file', 'pdf', 'quality'),
    (3, 1, 'courses/MAST20004/files/scan.pdf', 'scan.pdf', 'file', 'pdf', NULL),
    (4, 1, 'courses/MAST20004/files/Week_1/notes.pdf', 'notes.pdf', 'file', 'pdf', 'quality'),
    (5, 1, 'courses/MAST20004/files/Week_1/slides/deck.pdf', 'deck.pdf', 'file', 'pdf', 'quality'),
    (6, 1, 'courses/MAST20004/files/Week_2/notes.pdf', 'notes.pdf', 'file', 'pdf', NULL),
    (7, 1, 'courses/MAST20004/images/1.png', '1.png', 'image', 'png', NULL),
    (8, 2, 'courses/MAST2000X/files/other.md', 'other.md', 'file', 'md', NULL);`);
afterAll(() => sqlite.close());

// The statements `listMentionFolder` runs, against an in-memory SQLite.
function contents(path: string) {
  const q = mentionFolderQueries(path);
  return {
    folders: (sqlite.query(q.folders).all(...q.params) as MentionFolder[]).map((f) => [f.path, f.name]),
    files: (sqlite.query(q.files).all(...q.params) as MentionFile[]).map((f) => f.relative_path),
  };
}

test("a folder shows its own subfolders and the readable files directly in it", () => {
  expect(contents("courses/MAST20004/files/")).toEqual({
    folders: [
      ["courses/MAST20004/files/Week_1/", "Week_1"],
      ["courses/MAST20004/files/Week_2/", "Week_2"],
    ],
    // The unparsed scan is left out; what a subfolder holds is not this level's.
    files: ["courses/MAST20004/files/syllabus.pdf"],
  });
});

test("a subject's folder shows its top-level folders and root files", () => {
  expect(contents("courses/MAST20004/")).toEqual({
    folders: [
      ["courses/MAST20004/files/", "files"],
      ["courses/MAST20004/images/", "images"],
    ],
    files: ["courses/MAST20004/home.md"],
  });
});

test("levels nest, and a folder with nothing readable is empty", () => {
  expect(contents("courses/MAST20004/files/Week_1/")).toEqual({
    folders: [["courses/MAST20004/files/Week_1/slides/", "slides"]],
    files: ["courses/MAST20004/files/Week_1/notes.pdf"],
  });
  expect(contents("courses/MAST20004/images/")).toEqual({ folders: [], files: [] });
});

test("an underscore in the path is not a wildcard", () => {
  // `LIKE 'courses/MAST2000_/%'` would match both subjects.
  expect(contents("courses/MAST2000X/files/").files).toEqual(["courses/MAST2000X/files/other.md"]);
  expect(contents("courses/MAST2000_/files/")).toEqual({ folders: [], files: [] });
});

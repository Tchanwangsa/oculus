import { describe, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import {
  MAX_TAG_NAME,
  normaliseTagName,
  parseTagIds,
  taskBodyEdit,
} from "@/lib/planning/projects";
import { TASK_TAG_IDS_SQL, toTask, type RawTask } from "@/lib/planning/projects/rows";

describe("task body edits", () => {
  test("an unchanged body writes nothing, surrounding whitespace included", () => {
    expect(taskBodyEdit("Read week 3", "Read week 3")).toBeUndefined();
    expect(taskBodyEdit("  Read week 3\n\n", "Read week 3")).toBeUndefined();
    expect(taskBodyEdit("", null)).toBeUndefined();
    expect(taskBodyEdit(" \n", null)).toBeUndefined();
  });

  test("an edit writes the trimmed text", () => {
    expect(taskBodyEdit("Read week 4\n", "Read week 3")).toBe("Read week 4");
    expect(taskBodyEdit("First note", null)).toBe("First note");
  });

  test("emptying a body clears it", () => {
    expect(taskBodyEdit("   ", "Read week 3")).toBeNull();
  });
});

describe("task tag ids", () => {
  test("the JSON column parses to whole numbers, never null", () => {
    expect(parseTagIds("[2,3,1]")).toEqual([2, 3, 1]);
    expect(parseTagIds("[]")).toEqual([]);
    expect(parseTagIds(null)).toEqual([]);
    expect(parseTagIds("")).toEqual([]);
    expect(parseTagIds("not json")).toEqual([]);
    expect(parseTagIds('{"a":1}')).toEqual([]);
    expect(parseTagIds('[1,"2",null,1.5,4]')).toEqual([1, 4]);
  });

  test("toTask replaces the raw string and leaves the other columns", () => {
    const row = { id: 7, title: "t", tag_ids: "[3,1]", project_name: "P" };
    const mapped: unknown = toTask(row as never);
    expect(mapped).toEqual({ id: 7, title: "t", tag_ids: [3, 1], project_name: "P" });
    expect(toTask({ ...row, tag_ids: null } as never).tag_ids).toEqual([]);
  });

  test("the subquery yields registry order and [] when untagged", () => {
    const sqlite = new Database(":memory:");
    try {
      sqlite.exec(`CREATE TABLE projects (id INTEGER PRIMARY KEY, tags TEXT);
        CREATE TABLE project_tasks (id INTEGER PRIMARY KEY, title TEXT, project_id INTEGER);
        CREATE TABLE task_tags (id INTEGER PRIMARY KEY, position REAL, name TEXT);
        CREATE TABLE task_tag_links (task_id INT, tag_id INT, PRIMARY KEY (task_id, tag_id));
        INSERT INTO project_tasks VALUES (1, 'a', NULL), (2, 'b', NULL);
        INSERT INTO task_tags VALUES (1, 3, 'a'), (2, 1, 'b'), (3, 2, 'c');
        INSERT INTO task_tag_links VALUES (1, 1), (1, 2), (1, 3);`);
      const rows = sqlite
        .query(`SELECT t.*, ${TASK_TAG_IDS_SQL} FROM project_tasks t ORDER BY t.id`)
        .all() as RawTask[];
      expect(rows.map((r) => toTask(r).tag_ids)).toEqual([[2, 3, 1], []]);
    } finally {
      sqlite.close();
    }
  });
});

describe("tag names", () => {
  test("trims and collapses whitespace", () => {
    expect(normaliseTagName("  Group   work \n")).toBe("Group work");
  });

  test("refuses an empty name and one past the cap", () => {
    expect(() => normaliseTagName("   ")).toThrow();
    expect(normaliseTagName("x".repeat(MAX_TAG_NAME))).toHaveLength(MAX_TAG_NAME);
    expect(() => normaliseTagName("x".repeat(MAX_TAG_NAME + 1))).toThrow();
  });
});

test("shared tags backfill projects and follow edits without overwriting task tags", async () => {
  const sqlite = new Database(":memory:");
  try {
    sqlite.exec(`CREATE TABLE projects (id INTEGER PRIMARY KEY, tags TEXT, updated_at TEXT);
      CREATE TABLE project_tasks (id INTEGER PRIMARY KEY, project_id INTEGER);
      CREATE TABLE task_tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT COLLATE NOCASE UNIQUE, color TEXT, position REAL);
      CREATE TABLE task_tag_links (task_id INTEGER, tag_id INTEGER, PRIMARY KEY(task_id, tag_id));
      INSERT INTO task_tags VALUES (1, 'Assignment', 'indigo', 1), (2, 'Reading', 'green', 2);
      INSERT INTO projects VALUES (1, '["assignment","Custom"]', NULL), (2, '["Exam"]', NULL);
      INSERT INTO project_tasks VALUES (1, 1), (2, 1), (3, NULL);
      INSERT INTO task_tag_links VALUES (2, 2);`);
    sqlite.exec(await Bun.file('src-tauri/src/db/projects/shared_tags.sql').text());
    const ids = (id: number) => toTask(sqlite.query(`SELECT t.*, ${TASK_TAG_IDS_SQL} FROM project_tasks t WHERE t.id = ?`).get(id) as RawTask).tag_ids;
    const custom = sqlite.query("SELECT id FROM task_tags WHERE name = 'Custom'").get() as {id: number};
    expect(ids(1)).toEqual([1, custom.id]);
    expect(ids(2)).toEqual([2]);
    expect(ids(3)).toEqual([]);
    sqlite.run("UPDATE task_tags SET name = 'Renamed', color = 'teal' WHERE id = ?", [custom.id]);
    expect(sqlite.query("SELECT tags FROM projects WHERE id = 1").get()).toEqual({tags: '["assignment","Renamed"]'});
    expect(ids(1)).toEqual([1, custom.id]);
    sqlite.run("DELETE FROM task_tags WHERE id = ?", [custom.id]);
    expect(ids(1)).toEqual([1]);
    expect(sqlite.query("SELECT tags FROM projects WHERE id = 1").get()).toEqual({tags: '["assignment"]'});
    sqlite.run(`UPDATE projects SET tags = '["New project tag"]' WHERE id = 1`);
    const next = sqlite.query("SELECT id FROM task_tags WHERE name = 'New project tag'").get() as {id: number};
    expect(ids(1)).toEqual([next.id]);
    expect(ids(2)).toEqual([2]);
    sqlite.run("DELETE FROM task_tag_links WHERE task_id = 2");
    expect(ids(2)).toEqual([next.id]);
    sqlite.run("UPDATE project_tasks SET project_id = 2 WHERE id = 1");
    const exam = sqlite.query("SELECT id FROM task_tags WHERE name = 'Exam'").get() as {id: number};
    expect(ids(1)).toEqual([exam.id]);
    sqlite.run("UPDATE project_tasks SET project_id = NULL WHERE id = 1");
    expect(ids(1)).toEqual([]);
  } finally { sqlite.close(); }
});

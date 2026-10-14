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
      sqlite.exec(`CREATE TABLE project_tasks (id INTEGER PRIMARY KEY, title TEXT);
        CREATE TABLE task_tags (id INTEGER PRIMARY KEY, position REAL);
        CREATE TABLE task_tag_links (task_id INT, tag_id INT, PRIMARY KEY (task_id, tag_id));
        INSERT INTO project_tasks VALUES (1, 'a'), (2, 'b');
        INSERT INTO task_tags VALUES (1, 3), (2, 1), (3, 2);
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

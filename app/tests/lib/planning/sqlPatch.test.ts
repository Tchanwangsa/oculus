import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { patchColumns } from "@/lib/planning/sqlPatch";

test("patches bind values, skip undefined, preserve null/zero/empty, and target one row", async () => {
  const sqlite = new Database(":memory:");
  try {
    sqlite.exec(`CREATE TABLE project_tasks (
      id INTEGER PRIMARY KEY, title TEXT, body TEXT, estimate_minutes INTEGER,
      updated_at TEXT DEFAULT 'untouched'
    ); INSERT INTO project_tasks VALUES (1, 'one', 'body', 10, 'untouched');
       INSERT INTO project_tasks VALUES (2, 'two', 'body', 20, 'untouched');`);
    const db = { execute: async (sql: string, values: unknown[] = []) => {
      const result = sqlite.query(sql).run(...values as (string | number | null)[]);
      return { rowsAffected: result.changes, lastInsertId: Number(result.lastInsertRowid) };
    } };
    expect(await patchColumns(db, "project_tasks", 1, {
      title: undefined, body: null, estimate_minutes: 0,
    })).toBe(true);
    expect(sqlite.query("SELECT title, body, estimate_minutes FROM project_tasks WHERE id=1").get())
      .toEqual({ title: "one", body: null, estimate_minutes: 0 });
    const injection = "'; DELETE FROM project_tasks; --";
    await patchColumns(db, "project_tasks", 1, { title: injection, body: "" });
    expect(sqlite.query("SELECT title, body FROM project_tasks WHERE id=1").get())
      .toEqual({ title: injection, body: "" });
    expect(sqlite.query("SELECT * FROM project_tasks WHERE id=2").get()).toEqual({
      id: 2, title: "two", body: "body", estimate_minutes: 20, updated_at: "untouched",
    });
    expect(await patchColumns(db, "project_tasks", 2, { title: undefined })).toBe(false);
    expect(sqlite.query("SELECT updated_at FROM project_tasks WHERE id=2").get())
      .toEqual({ updated_at: "untouched" });
  } finally {
    sqlite.close();
  }
});

test("empty patches never execute, and SQL failures propagate", async () => {
  const db = { execute: async () => { throw new Error("write failed"); } };
  expect(await patchColumns(db, "projects", 1, {})).toBe(false);
  await expect(patchColumns(db, "projects", 1, { name: "changed" })).rejects.toThrow("write failed");
});

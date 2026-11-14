import { afterAll, expect, test } from "bun:test";
import { Database as SQLite } from "bun:sqlite";
import { previewsSql, THREADS_SQL } from "@/lib/harness/reads";

/** The columns the two reads touch, as the migrations leave them. */
const sqlite = new SQLite(":memory:");
sqlite.run(`CREATE TABLE harness_threads (id INTEGER PRIMARY KEY, title TEXT, updated_at TEXT NOT NULL DEFAULT '2026-11-15 00:00:00')`);
sqlite.run(`CREATE TABLE harness_items (id INTEGER PRIMARY KEY, thread_id INTEGER, kind TEXT, content TEXT, run_id INTEGER)`);
sqlite.run(`CREATE TABLE harness_runs (id INTEGER PRIMARY KEY, thread_id INTEGER, execution_status TEXT,
  initiating_item_id INTEGER, answer_item_id INTEGER, ended_at TEXT)`);
afterAll(() => sqlite.close());

const all = <T,>(sql: string, bindings: number[]) => sqlite.query(sql.replace(/\$(\d+)/g, "?$1")).all(...bindings) as T[];

test("a thread settles when a run that ran ends, never for one that waits or left the queue", () => {
  sqlite.run(`INSERT INTO harness_threads (id, title) VALUES (1, 'ran'), (2, 'only waiting'), (3, 'cancelled')`);
  sqlite.run(`INSERT INTO harness_runs (id, thread_id, execution_status, initiating_item_id, ended_at) VALUES
    (1, 1, 'completed', 10, '2026-11-15 01:00:00'),
    (2, 1, 'queued', NULL, NULL),
    (3, 2, 'queued', NULL, NULL),
    (4, 3, 'cancelled', NULL, '2026-11-15 03:00:00'),
    (5, 3, 'failed', NULL, '2026-11-15 04:00:00')`);
  const threads = all<{ id: number; settled_at: string | null }>(THREADS_SQL, [10]);
  const settled = Object.fromEntries(threads.map((t) => [t.id, t.settled_at]));
  expect(settled).toEqual({ 1: "2026-11-15 01:00:00", 2: null, 3: null });
});

test("a preview is the newest answer of a run that ran, beside runs that only wait", () => {
  sqlite.run(`INSERT INTO harness_items (id, thread_id, kind, content, run_id) VALUES
    (10, 1, 'user', 'asked', 1), (11, 1, 'assistant', 'commentary', 1), (12, 1, 'assistant', 'the answer', 1)`);
  sqlite.run(`UPDATE harness_runs SET answer_item_id = 12 WHERE id = 1`);
  const previews = all<{ thread_id: number; content: string }>(previewsSql(3), [1, 2, 3]);
  expect(previews).toEqual([{ thread_id: 1, content: "the answer" }]);
});

import { afterAll, expect, test } from "bun:test";
import { Database as SQLite } from "bun:sqlite";
import { flattenPreview, previewsSql, questionsSql, THREADS_SQL } from "@/lib/harness/reads";

/** The columns the two reads touch, as the migrations leave them. */
const sqlite = new SQLite(":memory:");
sqlite.run(`CREATE TABLE harness_threads (id INTEGER PRIMARY KEY, title TEXT, completed_at TEXT, updated_at TEXT NOT NULL DEFAULT '2026-11-15 00:00:00')`);
sqlite.run(`CREATE TABLE harness_items (id INTEGER PRIMARY KEY, thread_id INTEGER, kind TEXT, content TEXT, run_id INTEGER)`);
sqlite.run(`CREATE TABLE harness_runs (id INTEGER PRIMARY KEY, thread_id INTEGER, execution_status TEXT,
  initiating_item_id INTEGER, answer_item_id INTEGER, requested_at TEXT, started_at TEXT, ended_at TEXT)`);
sqlite.run(`CREATE TABLE harness_requests (id INTEGER PRIMARY KEY, thread_id INTEGER, status TEXT NOT NULL DEFAULT 'pending',
  responder TEXT NOT NULL DEFAULT 'live', kind TEXT NOT NULL DEFAULT 'approval', payload TEXT NOT NULL DEFAULT '{}')`);
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

test("a thread counts the requests that wait on the student, not those answered or ended", () => {
  sqlite.run(`INSERT INTO harness_requests (id, thread_id, status, responder) VALUES
    (1, 1, 'pending', 'live'), (2, 1, 'pending', 'live'), (3, 1, 'resolved', 'live'),
    (4, 2, 'expired', 'not_resumable'), (5, 2, 'pending', 'not_resumable')`);
  const threads = all<{ id: number; pending_requests: number }>(THREADS_SQL, [10]);
  const pending = Object.fromEntries(threads.map((t) => [t.id, t.pending_requests]));
  expect(pending).toEqual({ 1: 2, 2: 0, 3: 0 });
});

test("a preview is the newest answer of a run that ran, beside runs that only wait", () => {
  sqlite.run(`INSERT INTO harness_items (id, thread_id, kind, content, run_id) VALUES
    (10, 1, 'user', 'asked', 1), (11, 1, 'assistant', 'commentary', 1), (12, 1, 'assistant', 'the answer', 1)`);
  sqlite.run(`UPDATE harness_runs SET answer_item_id = 12 WHERE id = 1`);
  const previews = all<{ thread_id: number; content: string }>(previewsSql(3), [1, 2, 3]);
  expect(previews).toEqual([{ thread_id: 1, content: "the answer" }]);
});

test("a detached question counts as Input and an ended question does not", () => {
  sqlite.run(`INSERT INTO harness_requests (thread_id, kind, status, responder) VALUES
    (3, 'question', 'pending', 'message'), (3, 'question', 'cancelled', 'not_resumable')`);
  const row = all<{ id: number; pending_requests: number; pending_questions: number }>(THREADS_SQL, [10]).find((t) => t.id === 3);
  expect(row).toMatchObject({ pending_requests: 1, pending_questions: 1 });
});

test("a preview is the newest message from either side: the student's question beats an older answer", () => {
  sqlite.run(`INSERT INTO harness_threads (id, title) VALUES (20, 'asked again'), (21, 'answered last')`);
  sqlite.run(`INSERT INTO harness_runs (id, thread_id, execution_status, initiating_item_id, answer_item_id) VALUES
    (20, 20, 'completed', 200, 201), (21, 20, 'running', 202, NULL), (22, 21, 'completed', 210, 211)`);
  sqlite.run(`INSERT INTO harness_items (id, thread_id, kind, content, run_id) VALUES
    (200, 20, 'user', 'first question', 20), (201, 20, 'assistant', 'first answer', 20),
    (202, 20, 'user', 'a follow-up', 21), (203, 20, 'assistant', 'commentary mid-run', 21),
    (210, 21, 'user', 'why?', 22), (211, 21, 'assistant', 'because', 22), (212, 21, 'tool', 'ls', 22)`);
  const previews = all<{ thread_id: number; content: string }>(previewsSql(2), [20, 21]);
  expect(Object.fromEntries(previews.map((p) => [p.thread_id, p.content]))).toEqual({ 20: "a follow-up", 21: "because" });
});

test("a thread's question line is its newest waiting question's first question", () => {
  const payload = (text: string) => JSON.stringify({ questions: [{ id: "a", text }, { id: "b", text: "second" }] });
  sqlite.run(`INSERT INTO harness_requests (thread_id, kind, status, responder, payload) VALUES
    (20, 'question', 'resolved', 'live', ?1), (20, 'question', 'pending', 'live', ?2), (21, 'question', 'pending', 'not_resumable', ?3)`,
    [payload("answered already"), payload("Proof or intuition?"), payload("gone")]);
  const rows = all<{ thread_id: number; content: string }>(questionsSql(2), [20, 21]);
  expect(rows).toEqual([{ thread_id: 20, content: "Proof or intuition?" }]);
});

test("the list reads when the run in flight started, and nothing once none is", () => {
  sqlite.run(`UPDATE harness_runs SET requested_at = '2026-11-15 05:00:00', started_at = '2026-11-15 05:00:02' WHERE id = 21`);
  const rows = all<{ id: number; run_started_at: string | null }>(THREADS_SQL, [50]);
  const started = Object.fromEntries(rows.map((t) => [t.id, t.run_started_at]));
  expect(started[20]).toBe("2026-11-15 05:00:02");
  expect(started[21]).toBeNull();
});

test("a preview reads as one plain line", () => {
  expect(flattenPreview("## Plan\n- **Read** [week 7](/x.pdf)\n\n```py\nx\n```\nthen `run` it")).toBe("Plan Read week 7 then run it");
});

test("open reads exclude completed chats and an unlimited read reaches beyond 100", () => {
  sqlite.run("UPDATE harness_threads SET completed_at = '2026-11-16' WHERE id = 1");
  for (let id = 1000; id <= 1106; id++) sqlite.run("INSERT INTO harness_threads(id, title) VALUES (?1, 'open')", [id]);
  const threads = all<{ id: number }>(THREADS_SQL, [-1]);
  expect(threads.filter((t) => t.id >= 1000)).toHaveLength(107);
  expect(threads.some((t) => t.id === 1)).toBe(false);
  expect(threads[0].id).toBe(1106);
});

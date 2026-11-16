import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { readFileSync } from "node:fs";
import { completedSql, COMPLETED_COUNTS_SQL } from "@/lib/harness/completedReads";
import { SEARCH_THREADS_SQL, threadSearchBindings } from "@/lib/harness/searchThreads";

function fixture() {
  const db = new Database(":memory:");
  db.run(`PRAGMA foreign_keys = ON;
    CREATE TABLE subjects(id INTEGER PRIMARY KEY, code TEXT, name TEXT);
    CREATE TABLE harness_threads(id INTEGER PRIMARY KEY, title TEXT, subject_id INTEGER, updated_at TEXT);
    CREATE TABLE harness_items(id INTEGER PRIMARY KEY, thread_id INTEGER REFERENCES harness_threads(id) ON DELETE CASCADE, kind TEXT, content TEXT);
    INSERT INTO subjects VALUES (1,'MATH','Mathematics'), (2,'PHYS','Physics');`);
  for (const file of ["completed.sql", "search.sql"]) {
    db.run(readFileSync(new URL(`../../../src-tauri/src/harness/store/${file}`, import.meta.url), "utf8"));
  }
  return db;
}
const sql = (query: string) => query.replace(/\$(\d+)/g, "?$1");

test("completed pages read 10 then 25 with totals and subject/General folds", () => {
  const db = fixture();
  try {
    for (let id = 1; id <= 40; id++) db.run("INSERT INTO harness_threads VALUES (?1, ?2, ?3, ?4, ?5)", [id, `thread ${id}`, id % 2 ? 1 : null, "2026-01-01", id === 40 ? null : "2026-02-01"]);
    const queries = completedSql();
    const first = db.query(sql(queries.rows)).all(10, 0) as { id: number }[];
    const more = db.query(sql(queries.rows)).all(25, 10) as { id: number }[];
    expect(first.map((t) => t.id)).toEqual(Array.from({ length: 10 }, (_, i) => 39 - i));
    expect(more).toHaveLength(25);
    expect(more[0].id).toBe(29);
    expect(db.query(queries.count).get()).toEqual({ count: 39 });
    expect(db.query(COMPLETED_COUNTS_SQL).all()).toEqual([{ subject_id: null, count: 19 }, { subject_id: 1, count: 20 }]);
    for (const [subject, count] of [[null, 19], [1, 20], [2, 0]] as const) {
      const scoped = completedSql(subject);
      expect(db.query(sql(scoped.count)).get(subject)).toEqual({ count });
      const rows = db.query(sql(scoped.rows)).all(10, 0, subject) as { subject_id: number | null }[];
      expect(rows.every((t) => t.subject_id === subject)).toBe(true);
      expect(rows.length).toBe(Math.min(10, count));
    }
  } finally { db.close(); }
});

test("search returns one best hit per chat, title first then relevance and newest, including completed", () => {
  const db = fixture();
  try {
    db.run(`INSERT INTO harness_threads VALUES
      (1,'Eigen notes',1,'2026-01-01',NULL), (2,'Lecture',2,'2026-03-01','2026-03-02'),
      (3,'Other lecture',NULL,'2026-02-01',NULL), (4,'Noise',NULL,'2026-04-01',NULL);
      INSERT INTO harness_items VALUES
      (1,1,'user','eigenvalues and matrices'), (2,2,'assistant','eigenvalues and matrices'),
      (3,2,'user','eigenvectors and matrices'), (4,3,'user','eigenvalues and matrices'),
      (5,4,'tool','eigenvalues and matrices');`);
    const rows = db.query(sql(SEARCH_THREADS_SQL)).all(...threadSearchBindings("eigen")!) as { id: number; snippet: string; subject_code: string | null; completed_at: string | null }[];
    expect(rows.map((r) => r.id)).toEqual([1,2,3]);
    expect(rows[0].snippet).toBe("\uE000Eigen notes\uE001");
    expect(rows[1].snippet).toContain("\uE000eigen");
    expect(rows[1]).toMatchObject({ subject_code: "PHYS", completed_at: "2026-03-02" });
    expect(rows[2].subject_code).toBeNull();
  } finally { db.close(); }
});

test("search quotes operators, matches accents and prefixes, escapes LIKE wildcards, and caps at 50", () => {
  const db = fixture();
  try {
    expect(threadSearchBindings("  ")).toBeNull();
    expect(threadSearchBindings(' * " () ')).toBeNull();
    expect(threadSearchBindings('café OR content:NEAR("x") -y*')![0]).toBe('"café"* AND "OR"* AND "content"* AND "NEAR"* AND "x"* AND "y"*');
    for (let id = 1; id <= 60; id++) {
      db.run("INSERT INTO harness_threads VALUES (?1, 'Notes', NULL, '2026-01-01', NULL)", [id]);
      db.run("INSERT INTO harness_items VALUES (?1, ?1, 'assistant', 'café eigenvalues')", [id]);
    }
    expect(db.query(sql(SEARCH_THREADS_SQL)).all(...threadSearchBindings("cafe eigen")!)).toHaveLength(50);
    for (const query of ['" OR *', 'NEAR(a,b)', 'content:abc', 'x) NOT (y', 'a%_\\b']) expect(() => db.query(sql(SEARCH_THREADS_SQL)).all(...threadSearchBindings(query)!)).not.toThrow();
    db.run("INSERT INTO harness_threads VALUES (100, '100%_eigen', NULL, '2026-01-01', NULL), (101, '100xxeigen', NULL, '2026-01-01', NULL)");
    const rows = db.query(sql(SEARCH_THREADS_SQL)).all(...threadSearchBindings("100%_eigen")!) as { id: number }[];
    expect(rows.map((r) => r.id)).toEqual([100]);
  } finally { db.close(); }
});

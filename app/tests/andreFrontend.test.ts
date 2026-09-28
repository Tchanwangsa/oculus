import { afterAll, beforeEach, describe, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { compareTermsNewestFirst, TERM_RANK_SQL, termRank } from "../src/lib/terms";
import { getSubjects, searchPageText, setSubjectSelected, SNIP_OPEN, SNIP_CLOSE, upsertSubjects } from "../src/lib/db";
import { docPdfRelPath, isPdfBacked, PDF_BACKED_SQL_LIST, parsedMdRelPath } from "../src/lib/fileTypes";
import { SyncWriteQueue } from "../src/lib/syncWrites";
import { queueParseEvent } from "../src/lib/parseEvents";
import { useParseStore } from "../src/stores/parseStore";
import { usePipelineStore } from "../src/stores/pipelineStore";

const originalWindow = globalThis.window;
let db: Database;
beforeEach(() => {
  db?.close();
  db = new Database(":memory:");
  db.run(`CREATE TABLE subjects (
    id INTEGER PRIMARY KEY, code TEXT, name TEXT, term_name TEXT,
    is_current INTEGER, workflow_state TEXT, selected INTEGER
  )`);
  db.run(`CREATE TABLE sync_runs (finished_at TEXT, subject_codes TEXT, status TEXT)`);
  db.run(`CREATE TABLE files (
    id INTEGER PRIMARY KEY, subject_id INTEGER, relative_path TEXT,
    parse_status TEXT, parsed_at TEXT
  )`);
  useParseStore.setState({ statuses: {}, jobs: {} });
  usePipelineStore.setState({ items: {} });
  Object.assign(globalThis, { window: { __TAURI_INTERNALS__: {
    invoke: async (command: string, args: { db?: string; query: string; values: unknown[] }) => {
      if (command === "plugin:sql|load") return args.db;
      if (command === "library_database_url") return "sqlite::memory:";
      if (command === "plugin:sql|select") return db.query(args.query).all(...args.values as []);
      if (command === "plugin:sql|execute") {
        const result = db.query(args.query).run(...args.values as []);
        return [result.changes, result.lastInsertRowid];
      }
      throw new Error(`Unexpected IPC: ${command}`);
    },
  } } });
});
afterAll(() => { db.close(); Object.assign(globalThis, { window: originalWindow }); });

function seedLegacySubjects() {
  for (const [id, term, current] of [
    [1, "2026 Summer Term", 1], [2, "2026 Semester 1", 0],
    [3, "2026 Winter Term", 0], [4, "2026 Semester 2", 0],
  ] as const) {
    db.run("INSERT INTO subjects VALUES (?, ?, ?, ?, ?, 'available', ?)",
      [id, `TEST${id}`, `Subject ${id}`, term, current, current]);
  }
}

describe("academic term order and initial sync defaults", () => {
  test("newest-first order agrees in TypeScript and SQLite for labels and codes", () => {
    const names = ["2026 Summer Term", "2026 Semester 1", "2026 Winter Term", "2026 Semester 2"];
    expect([...names].sort(compareTermsNewestFirst)).toEqual([...names].reverse());
    for (const [rank, aliases] of [[0, ["2026_SUM", "2026-SUMMER"]], [1, ["2026_SM1"]],
      [2, ["2026_WIN", "2026-WINTER"]], [3, ["2026_SM2"]]] as const) {
      for (const name of aliases) {
        expect(termRank(name)).toBe(rank);
        expect(db.query(`SELECT ${TERM_RANK_SQL("$1")} AS rank`).get(name)).toEqual({ rank });
      }
    }
    expect(compareTermsNewestFirst("2027 Summer Term", "2026 Semester 2")).toBeLessThan(0);
  });

  test("repairs a Summer-only obsolete default to Semester 2 and orders past terms", async () => {
    seedLegacySubjects();
    const rows = await getSubjects();
    expect(rows.map((s) => s.id)).toEqual([4, 3, 2, 1]);
    expect(rows.filter((s) => s.is_current).map((s) => s.id)).toEqual([4]);
    expect(rows.filter((s) => s.selected).map((s) => s.id)).toEqual([4]);
    expect(db.query("SELECT id FROM subjects WHERE selected=1").all()).toEqual([{ id: 4 }]);
  });

  test("preserves custom selection and later deliberate checkbox edits", async () => {
    seedLegacySubjects();
    db.run("UPDATE subjects SET selected = CASE WHEN id=2 THEN 1 ELSE 0 END");
    expect((await getSubjects()).filter((s) => s.selected).map((s) => s.id)).toEqual([2]);
    await setSubjectSelected(2, false);
    await setSubjectSelected(1, true);
    expect((await getSubjects()).filter((s) => s.selected).map((s) => s.id)).toEqual([1]);
    await setSubjectSelected(1, false);
    expect((await getSubjects()).filter((s) => s.selected)).toEqual([]);
  });

  test("new courses select Semester 2 even when a legacy Canvas event flags Summer", async () => {
    await upsertSubjects([
      { id: 1, course_code: "SUM", name: "Summer", term: { name: "2026 Summer Term" }, workflow_state: "available", _oculus_is_current: true },
      { id: 2, course_code: "SM1", name: "Semester 1", term: { name: "2026 Semester 1" }, workflow_state: "available", _oculus_is_current: false },
      { id: 3, course_code: "SM2", name: "Semester 2", term: { name: "2026 Semester 2" }, workflow_state: "available", _oculus_is_current: false },
    ]);
    expect((await getSubjects()).filter((s) => s.selected).map((s) => s.id)).toEqual([3]);
  });

  test("refetching course metadata preserves an explicit past-subject selection", async () => {
    seedLegacySubjects();
    db.run("UPDATE subjects SET selected = CASE WHEN id=2 THEN 1 ELSE 0 END");
    await upsertSubjects([
      { id: 1, course_code: "TEST1", name: "Summer", term: { name: "2026 Summer Term" }, workflow_state: "available", _oculus_is_current: false },
      { id: 2, course_code: "TEST2", name: "Semester 1", term: { name: "2026 Semester 1" }, workflow_state: "available", _oculus_is_current: false },
      { id: 3, course_code: "TEST3", name: "Winter", term: { name: "2026 Winter Term" }, workflow_state: "available", _oculus_is_current: false },
      { id: 4, course_code: "TEST4", name: "Semester 2", term: { name: "2026 Semester 2" }, workflow_state: "available", _oculus_is_current: true },
    ]);
    expect((await getSubjects()).filter((s) => s.selected).map((s) => s.id)).toEqual([2]);
  });
});

test("Excel downloads and uploads use derived PDFs in every pipeline query", () => {
  for (const filename of ["Budget.XLS", "学生 notes.xlsx"]) {
    const file = { filename, relative_path: `courses/TEST/uploads/${filename}` };
    expect(isPdfBacked(filename)).toBe(true);
    expect(docPdfRelPath(file)).toBe(`${file.relative_path}.pdf`);
    expect(parsedMdRelPath(file)).toBe(`${file.relative_path}.md`);
    expect(db.query(`SELECT lower($1) IN ${PDF_BACKED_SQL_LIST} AS supported`).get(filename.split(".").pop()!)).toEqual({ supported: 1 });
  }
  expect(isPdfBacked("notes.csv")).toBe(false);
});

describe("full-text document search", () => {
  beforeEach(() => {
    db.run("ALTER TABLE files ADD COLUMN filename TEXT");
    db.run("ALTER TABLE files ADD COLUMN category TEXT");
    db.run("CREATE TABLE pages (id INTEGER PRIMARY KEY, file_id INTEGER, page_no INTEGER, markdown TEXT)");
    db.run("CREATE VIRTUAL TABLE pages_fts USING fts5(markdown, content='pages', content_rowid='id', tokenize='unicode61 remove_diacritics 2')");
    db.run("INSERT INTO subjects VALUES (1, 'PAST', 'Past subject', '2025 Semester 2', 0, 'available', 0), (2, 'CURRENT', 'Current subject', '2026 Semester 2', 1, 'available', 1)");
    for (const [id, subject] of [[1, 1], [2, 2], [3, 2]]) {
      db.run("INSERT INTO files (id, subject_id, relative_path, filename, category) VALUES (?, ?, ?, ?, 'lecture')",
        [id, subject, `courses/${subject}/files/${id}.pdf`, `${id}.pdf`]);
    }
  });

  function page(id: number, fileId: number, pageNo: number, markdown: string) {
    db.run("INSERT INTO pages VALUES (?, ?, ?, ?)", [id, fileId, pageNo, markdown]);
    db.run("INSERT INTO pages_fts (rowid, markdown) VALUES (?, ?)", [id, markdown]);
  }

  test("returns one best page per file with its matching snippet", async () => {
    page(1, 1, 1, "Eigenvalues introduction " + "unrelated ".repeat(80));
    page(2, 1, 2, "Eigenvalues eigenvalues important");
    page(3, 2, 1, "Eigenvalues distinct result");
    const hits = await searchPageText("Eigenvalues");
    expect(hits.map((hit) => hit.file_id)).toEqual([1, 2]);
    expect(hits[0]).toMatchObject({ page_no: 2, subject_code: "PAST", filename: "1.pdf" });
    expect(hits[0].snippet).toContain(`${SNIP_OPEN}Eigenvalues${SNIP_CLOSE}`);
    expect(hits[0].snippet).toContain("important");
    expect(hits[0].snippet).not.toContain("introduction");
  });

  test("tied page scores choose the earliest page and retain its own snippet", async () => {
    page(1, 1, 3, "Eigenvalues later theorem");
    page(2, 1, 1, "Eigenvalues earlier theorem");
    const hits = await searchPageText("Eigen");
    expect(hits).toHaveLength(1);
    expect(hits[0].page_no).toBe(1);
    expect(hits[0].snippet).toContain("earlier");
    expect(hits[0].snippet).not.toContain("later");
  });

  test("tied files prefer current subjects, respect the limit, and ignore orphan index rows", async () => {
    for (const id of [3, 1, 2]) page(id, id, 1, "Eigenvalues identical theorem");
    // A stale index entry must not survive the join to its deleted page.
    page(99, 1, 99, "Eigenvalues stale theorem");
    db.run("DELETE FROM pages WHERE id=99");
    expect((await searchPageText("Eigenvalues", 2)).map((hit) => hit.file_id)).toEqual([2, 3]);
    expect((await searchPageText("Eigenvalues", 10)).map((hit) => hit.file_id)).toEqual([2, 3, 1]);
  });
});

describe("sync metadata completion barrier", () => {
  test("a quality completion behind a blocked write ignores its trailing running heartbeat", async () => {
    const queue = new SyncWriteQueue();
    const path = "courses/TEST/files/slides.pdf";
    db.run("INSERT INTO files VALUES (1, 1, ?, 'running', NULL)", [path]);
    useParseStore.getState().update({ relative_path: path, subject_id: 1, status: "running" });
    usePipelineStore.getState().touch(path, 1, { parse: "active" });
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    void queue.enqueue(7, "prior file write", () => gate, () => {});
    const parsed: string[] = [];
    const errors: string[] = [];
    for (const status of ["quality", "running"]) {
      void queueParseEvent(queue, 7, { relative_path: path, subject_id: 1, status },
        (_, savedPath) => parsed.push(savedPath), (message) => errors.push(message));
    }
    await Promise.resolve();
    expect(useParseStore.getState().statuses[path]).toBe("running");
    release();
    await queue.drain();
    expect(db.query("SELECT parse_status FROM files WHERE id=1").get()).toEqual({ parse_status: "quality" });
    expect(useParseStore.getState().statuses[path]).toBe("quality");
    expect(useParseStore.getState().jobs[path]).toBeUndefined();
    expect(usePipelineStore.getState().items[path].parse).toBe("done");
    expect(usePipelineStore.getState().items[path].embed).toBe("pending");
    expect(parsed).toEqual([path]);
    expect(errors).toEqual([]);
    expect(queue.takeFailure(7)).toBeNull();
  });

  test("a failed parse-state commit reports failure without claiming quality or starting embed", async () => {
    const queue = new SyncWriteQueue();
    const path = "courses/TEST/files/slides.pdf";
    db.run("DROP TABLE files");
    const parsed: string[] = [];
    const errors: string[] = [];
    const saved = await queueParseEvent(queue, 7,
      { relative_path: path, subject_id: 1, status: "quality" },
      (_, savedPath) => parsed.push(savedPath), (message) => errors.push(message));
    expect(saved).toBe(false);
    expect(parsed).toEqual([]);
    expect(useParseStore.getState().statuses[path]).toBe("error");
    expect(usePipelineStore.getState().items[path].parse).toBe("error");
    expect(errors[0]).toContain("no such table");
    expect(queue.takeFailure(7)).toContain("no such table");
  });

  test("waits for delayed file writes and work appended while draining", async () => {
    const queue = new SyncWriteQueue();
    const order: string[] = [];
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    void queue.enqueue(7, "first file", async () => { await gate; order.push("file"); }, () => {});
    const finished = queue.drain().then(() => order.push("complete"));
    void queue.enqueue(7, "last ledger row", async () => { order.push("ledger"); }, () => {});
    await Promise.resolve();
    expect(order).toEqual([]);
    release();
    await finished;
    expect(order).toEqual(["file", "ledger", "complete"]);
    expect(queue.takeFailure(7)).toBeNull();
  });

  test("a corrupt file insert cannot report a success ledger, and later writes still drain", async () => {
    const queue = new SyncWriteQueue();
    const actions: string[] = [];
    void queue.enqueue(7, "courses/TEST/file.pdf", async () => {
      await Promise.reject(new Error("database disk image is malformed"));
      actions.push("success ledger");
    }, (message) => actions.push(message));
    void queue.enqueue(7, "next file", async () => { actions.push("next file saved"); }, () => {});
    await queue.drain();
    expect(actions).not.toContain("success ledger");
    expect(actions.at(-1)).toBe("next file saved");
    expect(queue.takeFailure(7)).toContain("database disk image is malformed");
    expect(queue.takeFailure(7)).toBeNull();
    expect(queue.takeFailure(8)).toBeNull();
  });
});

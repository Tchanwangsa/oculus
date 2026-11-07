import { afterAll, describe, expect, spyOn, test } from "bun:test";
import { createQueryManager, type QueryStatementInfo } from "@openuidev/lang-core";
const globals = globalThis as { window?: unknown; document?: unknown };
const added = (["window", "document"] as const).filter(k => !(k in globals));
for (const k of added) globals[k] = Object.assign(new EventTarget(), { documentElement: { style: {} } });
afterAll(() => { for (const k of added) delete globals[k]; });
const { createQueryTools, toolProvider } = await import("@/components/markdown/openui/query");
const { localReads } = await import("@/components/markdown/openui/query/reads");
const { subscribeQueryRefresh } = await import("@/components/markdown/openui/query/refresh");
const { isFenceFailure } = await import("@/components/markdown/openui/query/errors");
const { MAX_ROWS } = await import("@/components/markdown/openui/query/shared");

type Reads = typeof localReads;
const now = Date.parse("2026-11-08T00:00:00Z");
const subject = { id: 1, code: "MATH1_2026_SM2", name: "Math", term_name: "Semester 2", is_current: true, selected: true, last_synced_at: null };
const task = { id: 1, project_id: null, parent_id: null, title: "Study", body: null, column_id: "todo", position: 1, starts_at: null, due_at: "2026-11-09", estimate_minutes: 30, done_at: null, tag_ids: [2], source: "manual", created_at: "", updated_at: "" };
const project = { id: 12, subject_id: 1, subject_code: subject.code, name: "Exam", brief: null, status: "active", starts_at: null, due_at: null, columns: [{ id: "todo", name: "To do", kind: "todo" }], tags: [], event_id: null, position: 1, source: "manual", created_at: "", updated_at: "" };
const event = { id: "canvas", subject_id: 1, subject_code: subject.code, kind: "due", title: "Exam", start_at: "2026-11-09T00:00:00Z", end_at: null, all_day: 0, location: "Room", url: null, description: "Private Canvas description" };
const lecture = { id: "lecture", subject_id: 1, subject_code: subject.code, title: "Lecture", date: "2026-11-08", duration_seconds: 60, video_path: "/local/video", transcript_path: null };
const note = { id: 1, subject_id: 1, topic_ids: [5], title: "CLT", body: "Means converge", tags: [] };
const item = { id: 1, subject_id: 1, topic_ids: [5], prompt: "Why?", status: "active", due: "2026-11-07T00:00:00Z" };
const repeat = <T,>(row: T) => Array.from({ length: 450 }, (_, i) => ({ ...row, id: i + 1 }));
function fixture(overrides: Partial<Reads> = {}) {
  return { getSubjects: async () => repeat(subject), getAllTasks: async () => repeat(task), getTasks: async () => repeat(task),
    getProject: async () => project, getProjects: async () => repeat(project), getTaskCounts: async () => new Map([[12, { total: 7, done: 3 }]]),
    getFileCounts: async () => new Map([[1, 2]]), getTaskTags: async () => [{ id: 2, name: "Exam" }],
    getCalendarEvents: async () => repeat(event).map(e => ({ ...e, id: String(e.id) })), getLocalEvents: async () => [],
    getAllLectures: async () => repeat(lecture).map(l => ({ ...l, id: String(l.id) })), getNotes: async () => repeat(note), getItems: async () => repeat(item), ...overrides } as unknown as Reads;
}
const invalid: Record<string, object[]> = {
  tasks: [{ project: -1 }, { project: 1.5 }, { project: 1, unfiled: true }, { column: "todo" }, { due_before: "tomorrow" }, { due_before: "2026-99-99" }, { tag: "" }],
  projects: [{ subject: false }, { subject: "" }, { personal: true, subject: 1 }, { archived: "yes" }],
  calendar: [{ days: 366 }, { days: 1.5 }, { past: "yes" }, { subject: -1 }],
  upcoming: [{ days: -1 }, { due: 1 }], deadlines: [{ days: Infinity }, { subject: {} }],
  lectures: [{ subject: 0 }, { subject: "" }], "revision due": [{ topic: 0 }, { subject: false }], "cheat notes": [{ topic: 1.5 }, { subject: "" }], subjects: [{ subject: 1 }],
};

describe("local Query tools", () => {
  const tools = createQueryTools(fixture(), () => now);
  for (const [name, tool] of Object.entries(tools)) {
    test(`${name}: validates every argument before reading`, async () => {
      for (const input of [...invalid[name], { limit: 0 }, { limit: -1 }, { limit: 1.5 }, { limit: "3" }, { unexpected: true }, null, []]) {
        await expect(tool.run(input as never)).rejects.toThrow();
      }
    });
    test(`${name}: defaults to 100 rows and caps output at ${MAX_ROWS}`, async () => {
      expect(await tool.run({})).toHaveLength(100);
      expect(await tool.run({ limit: 2 })).toHaveLength(2);
      expect(await tool.run({ limit: 999999 })).toHaveLength(MAX_ROWS);
    });
    test(`${name}: refreshes only on its events and unsubscribes`, () => {
      const target = new EventTarget();
      const seen: string[] = [];
      const queries: QueryStatementInfo[] = [{ statementId: "rows", toolAST: { k: "Str", v: name }, argsAST: null, defaultsAST: null, refreshAST: null, complete: true }];
      const stop = subscribeQueryRefresh(target, queries, plan => {
        expect(plan.steps[0]).toEqual({ type: "run", statementId: "rows", refType: "query" });
        seen.push("rows");
      }, tools);
      target.dispatchEvent(new Event("unrelated"));
      expect(seen).toHaveLength(0);
      tool.events.forEach(event => target.dispatchEvent(new Event(event)));
      expect(seen).toHaveLength(tool.events.length);
      stop();
      tool.events.forEach(event => target.dispatchEvent(new Event(event)));
      expect(seen).toHaveLength(tool.events.length);
    });
    test(`${name}: failures reach the runtime QueryError channel and retry succeeds`, async () => {
      let failing = true;
      const manager = createQueryManager({ callTool: async () => { if (failing) throw new Error("Local database unavailable"); return tool.run({ limit: 2 }); } });
      const quiet = spyOn(console, "error").mockImplementation(() => {});
      try {
        manager.evaluateQueries([{ statementId: "rows", toolName: name, args: {}, defaults: [], complete: true }]);
        await Bun.sleep(0);
        expect(manager.getSnapshot().__openui_errors).toMatchObject([{ source: "query", statementId: "rows", toolName: name, code: "tool-error" }]);
        failing = false;
        manager.invalidate(["rows"]);
        await Bun.sleep(0);
        expect(manager.getSnapshot().__openui_errors).toHaveLength(0);
        expect(manager.getResult("rows")).toHaveLength(2);
      } finally { manager.dispose(); quiet.mockRestore(); }
    });
  }
  test("provider has exactly the read tools and no inherited callable properties", () => {
    expect(Object.keys(toolProvider).sort()).toEqual(Object.keys(tools).sort());
    expect(toolProvider.constructor).toBeUndefined();
    expect(toolProvider.Mutation).toBeUndefined();
  });
  test("CLI task, subject, lecture, project and calendar JSON fields are retained", async () => {
    expect((await tools.tasks.run({ limit: 1 }))[0]).toEqual({ ...task, file_count: 2 });
    expect((await tools.subjects.run({ limit: 1 }))[0]).toEqual({ id: 1, code: subject.code, name: "Math", term: "Semester 2", current: true, selected: true, last_synced_at: null });
    expect((await tools.lectures.run({ limit: 1 }))[0]).toMatchObject({ has_video: true, has_transcript: false });
    const projects = createQueryTools(fixture({ getProjects: async () => [project] } as unknown as Partial<Reads>));
    expect((await projects.projects.run({}))[0]).toMatchObject({ tasks_total: 7, tasks_done: 3 });
    expect((await tools.calendar.run({ limit: 1 }))[0]).toMatchObject({ subject: subject.code, all_day: false, notes: null, start_at: event.start_at });
  });
  test("calendar keeps local personal events, date-only today, ordering and deadline filters", async () => {
    const today = new Date(now).toLocaleDateString("en-CA");
    const tools = createQueryTools(fixture({ getCalendarEvents: async () => [event, { ...event, id: "past", start_at: "2026-11-07T00:00:00Z" }],
      getLocalEvents: async () => [{ ...event, id: 2, subject_id: null, subject_code: null, kind: "note", start_at: today, all_day: 1, notes: "Read" }] } as unknown as Partial<Reads>), () => now);
    expect(await tools.calendar.run({})).toHaveLength(2);
    expect(await tools.deadlines.run({})).toHaveLength(1);
    expect(await tools.calendar.run({ past: true })).toHaveLength(3);
    expect((await tools.calendar.run({}))[0]).toMatchObject({ id: "local_2", subject: null, all_day: true, notes: "Read", starts_local: today });
  });
  test("revision due excludes future, draft and suspended items", async () => {
    const tools = createQueryTools(fixture({ getItems: async () => [item, { ...item, id: 2, status: "draft" }, { ...item, id: 3, status: "suspended" }, { ...item, id: 4, due: "2026-12-01T00:00:00Z" }] } as unknown as Partial<Reads>), () => now);
    expect(await tools["revision due"].run({})).toEqual([item]);
    expect(await tools["revision due"].run({ topic: 6 })).toEqual([]);
  });
  test("unknown and ambiguous subjects error in place; invalid args never reach DB", async () => {
    let reads = 0;
    const tools = createQueryTools(fixture({ getAllTasks: async () => { reads++; return []; } }));
    await expect(tools.tasks.run({ project: -1 })).rejects.toThrow();
    expect(reads).toBe(0);
    await expect(tools.lectures.run({ subject: "UNKNOWN" })).rejects.toThrow("Unknown subject");
    await expect(tools.projects.run({ subject: "MATH1" })).rejects.toThrow("Ambiguous subject");
  });
});

test("QueryError and its empty-prop cascades never fail the fence", () => {
  const query = { source: "query", code: "tool-error", message: "Unavailable" } as const;
  const cascade = { source: "runtime", code: "runtime-error", message: "Empty rows" } as const;
  expect(isFenceFailure([query])).toBe(false);
  expect(isFenceFailure([query, cascade])).toBe(false);
  expect(isFenceFailure([cascade])).toBe(true);
  expect(isFenceFailure([{ ...cascade, severity: "warning" }])).toBe(false);
});

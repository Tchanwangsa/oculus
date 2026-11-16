import { beforeEach, describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { HarnessItem, HarnessRun, SteerDelivery } from "@/lib/harness";

// The row's import graph reaches modules that listen on `window` and
// `document` at load; they get inert stand-ins for the import, then the
// globals go back so no other test sees a half DOM.
const globals = globalThis as { window?: unknown; document?: unknown };
const before = { window: globals.window, document: globals.document };
const inert = { addEventListener: () => {}, removeEventListener: () => {} };
globals.window ??= inert;
globals.document ??= { ...inert, documentElement: { style: {} } };
const { SteerRow } = await import("@/components/harness/timeline/SteerRow");
const { buildRows, foldLabel, foldOpens, rowItems } = await import("@/components/harness/timeline/rows");
const { useHarnessStore } = await import("@/stores/chat/harnessStore");
const { TooltipProvider } = await import("@/components/ui/tooltip");
Object.assign(globals, before);
type ViewRow = ReturnType<typeof buildRows>[number];
type WorkFold = Extract<ViewRow, { kind: "work" }>;

const THREAD = 3;
const RUN = 1;
/** Seconds after the run's start, as SQLite writes `created_at`. */
const at = (s: number) => `2026-11-16 10:00:${String(s).padStart(2, "0")}`;
const START = Date.parse("2026-11-16T10:00:00Z");

function row(id: number, kind: HarnessItem["kind"], s: number, patch: Partial<HarnessItem> = {}): HarnessItem {
  return {
    id, thread_id: THREAD, kind, ref_id: null, content: `${kind} ${id}`,
    meta: kind === "tool" ? JSON.stringify({ kind: "bash", ok: true }) : null,
    created_at: at(s), run_id: RUN, ...patch,
  };
}
const steer = (id: number, s: number, delivery: SteerDelivery = "delivered") =>
  row(id, "steer", s, { content: `steer ${id}`, meta: JSON.stringify({ message: 100 + id, client: `c-${id}`, delivery }) });

const running: HarnessRun = { id: RUN, status: "running", delivery: "unknown", answer_item_id: null, started_ms: START, ended_ms: null, provider: "codex" };
const settled = (status: HarnessRun["status"], answer: number | null): HarnessRun => ({
  ...running, status, delivery: "delivered", answer_item_id: answer, ended_ms: START + 30_000,
});

/** Each row as `kind:id`, a fold as `work:<ids>`. */
const shape = (rows: ViewRow[]) =>
  rows.map((r) => (r.kind === "work" ? `work:${r.items.map((i) => i.id).join(",")}` : `${r.kind}:${rowItems(r)[0].id}`));
const folds = (rows: ViewRow[]) => rows.filter((r): r is WorkFold => r.kind === "work");
const answerOf = (rows: ViewRow[]) => rows.find((r) => r.kind === "item" && r.role);

beforeEach(() => {
  useHarnessStore.getState().flushLive();
  useHarnessStore.setState({ threads: [], items: { [THREAD]: [] }, runs: {}, live: {}, holds: { [THREAD]: 1 }, queued: {}, contextDrift: {} });
});

describe("a steer row in the running run", () => {
  // Codex `codex-steer-tool`'s shape: a reply, a command, the steer, then
  // the corrected work and answer.
  const items = [
    row(1, "user", 0),
    row(2, "assistant", 2),
    row(3, "tool", 3),
    steer(4, 5),
    row(5, "tool", 8),
    row(6, "assistant", 12),
  ];

  test("sits at its place in order, the work before it and after it in folds of their own", () => {
    const rows = buildRows(items, true, { runs: { [RUN]: running } });
    expect(shape(rows)).toEqual(["item:1", "work:2,3", "item:4", "work:5", "item:6"]);
  });

  test("the work before the steer is closed and ended at the steer; the work after it is the live one", () => {
    const [first, after] = folds(buildRows(items, true, { runs: { [RUN]: running } }));
    expect([first.id, first.cut, first.state, foldOpens(first.state)]).toEqual([`w-${RUN}`, "steer", "completed", false]);
    expect([first.startedMs, first.endedMs]).toEqual([START, START + 5_000]);
    expect(foldLabel(first)).toBe("Worked for 5s");
    expect([after.id, after.cut, after.state, after.endedMs]).toEqual([`w-${RUN}-4`, undefined, "running", null]);
    expect(after.startedMs).toBe(START + 5_000);
  });

  test("only a reply after the latest steer can be the provisional answer", () => {
    const rows = buildRows(items, true, { runs: { [RUN]: running } });
    expect(answerOf(rows)).toMatchObject({ item: { id: 6 }, role: "provisional" });
    // The steer just went in: the reply before it is work, and nothing answers yet.
    const fresh = buildRows(items.slice(0, 4), true, { runs: { [RUN]: running } });
    expect(shape(fresh)).toEqual(["item:1", "work:2,3", "item:4"]);
    expect(answerOf(fresh)).toBeUndefined();
  });

  test("two steers in a row: three stretches, each fold its own", () => {
    const two = [...items.slice(0, 4), steer(7, 6), row(8, "tool", 9), row(9, "assistant", 14)];
    const rows = buildRows(two, true, { runs: { [RUN]: running } });
    expect(shape(rows)).toEqual(["item:1", "work:2,3", "item:4", "item:7", "work:8", "item:9"]);
    expect(folds(rows).map((f) => f.id)).toEqual([`w-${RUN}`, `w-${RUN}-7`]);
  });

  test("a settled run keeps one answer, its folds all closed, the last ending with the run", () => {
    const rows = buildRows(items, false, { runs: { [RUN]: settled("completed", 6) } });
    expect(shape(rows)).toEqual(["item:1", "work:2,3", "item:4", "work:5", "item:6"]);
    expect(answerOf(rows)).toMatchObject({ item: { id: 6 }, role: "answer" });
    expect(rows.filter((r) => r.kind === "item" && r.role).length).toBe(1);
    const [first, last] = folds(rows);
    expect(foldOpens(first.state) || foldOpens(last.state)).toBe(false);
    expect(foldLabel(last)).toBe("Worked for 25s");
  });

  test("a stopped run opens only the stretch it was stopped in", () => {
    const rows = buildRows([...items.slice(0, 5), row(6, "interrupted", 12)], false, { runs: { [RUN]: settled("interrupted", null) } });
    const [first, last] = folds(rows);
    expect([foldOpens(first.state), foldOpens(last.state)]).toEqual([false, true]);
    expect(foldLabel(last)).toBe("You stopped after 25s");
  });

  test("the store's rows draw the same: a steer pushed live, then delivered", () => {
    const fold = (e: never) => useHarnessStore.getState().apply(e);
    const env = (revision: number, itemId: number | null, event: object) =>
      ({ threadId: THREAD, provider: "codex", itemId, runId: RUN, revision, event }) as never;
    fold(env(2, 1, { type: "user_message", text: "question" }));
    fold(env(3, 2, { type: "tool_started", id: "t1", name: "Bash", input: "sleep 20" }));
    fold(env(4, 3, { type: "steer", text: "in metres", message: 9, client: "c", delivery: "sending" }));
    fold(env(5, null, { type: "steer_updated", item_id: 3, message: 9, delivery: "delivered" }));
    const s = useHarnessStore.getState();
    const rows = buildRows(s.items[THREAD], true, { runs: s.runs[THREAD] });
    expect(shape(rows)).toEqual(["item:1", "work:2", "item:3"]);
  });

  test("a steer that missed its turn leaves the run: no steer row, nothing split", () => {
    const fold = (e: never) => useHarnessStore.getState().apply(e);
    const env = (revision: number, itemId: number | null, event: object) =>
      ({ threadId: THREAD, provider: "codex", itemId, runId: RUN, revision, event }) as never;
    fold(env(2, 1, { type: "user_message", text: "question" }));
    fold(env(3, 2, { type: "tool_started", id: "t1", name: "Bash", input: "ls" }));
    fold(env(4, 3, { type: "steer", text: "late", message: 9, client: "c", delivery: "sending" }));
    fold(env(5, null, { type: "steer_updated", item_id: 3, message: 9, delivery: "requeued" }));
    const s = useHarnessStore.getState();
    expect(shape(buildRows(s.items[THREAD], true, { runs: s.runs[THREAD] }))).toEqual(["item:1", "work:2"]);
  });
});

describe("restart steering", () => {
  const items = [row(1, "user", 0), row(2, "assistant", 3), row(3, "tool", 4), steer(4, 6), row(5, "assistant", 15)];

  test("on a restart-class bridge the superseded attempt folds as partial work", () => {
    const rows = buildRows(items, false, { runs: { [RUN]: settled("completed", 5) }, restarts: () => true });
    const [first] = folds(rows);
    expect(first.cut).toBe("restart");
    expect(foldLabel(first)).toBe("Restarted after 6s");
    expect(foldOpens(first.state)).toBe(false);
    expect(answerOf(rows)).toMatchObject({ item: { id: 5 }, role: "answer" });
  });

  test("a steer still waiting on its restart marks the stretch before it, on any bridge", () => {
    const waiting = [...items.slice(0, 3), steer(4, 6, "restart")];
    const [first] = folds(buildRows(waiting, true, { runs: { [RUN]: running } }));
    expect(first.cut).toBe("restart");
  });

  test("a native steer's earlier stretch is plain work", () => {
    const [first] = folds(buildRows(items, false, { runs: { [RUN]: settled("completed", 5) }, restarts: () => false }));
    expect(first.cut).toBe("steer");
  });
});

describe("the steer row's delivery", () => {
  const draw = (delivery: SteerDelivery) => renderToStaticMarkup(
    <TooltipProvider>
      <SteerRow item={steer(4, 5, delivery)} />
    </TooltipProvider>,
  );

  test("its words, as the student's message, with no rewind or edit", () => {
    const html = draw("delivered");
    expect(html).toContain("steer 4");
    expect(html).not.toContain("Rewind");
    expect(html).not.toContain("Edit");
  });

  test("sending and written say so with a spinner; delivered is quiet", () => {
    expect(draw("sending")).toContain("Sending into the turn…");
    expect(draw("written")).toContain("animate-spin");
    expect(draw("delivered")).not.toContain('role="status"');
  });

  test("unknown says it may not have arrived and will not be resent", () => {
    const html = draw("unknown");
    expect(html).toContain("This may not have reached the agent. It won&#x27;t be sent again.");
    expect(html).toContain("text-warning");
  });

  test("restart says the turn is stopping to restart", () => {
    expect(draw("restart")).toContain("Stopping the turn to restart with this…");
  });
});

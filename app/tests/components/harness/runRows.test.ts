import { beforeEach, describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ReactMarkdown from "react-markdown";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { runFromRow, type RunRow } from "@/lib/harness/runs";
import type { HarnessEnvelope, HarnessItem, HarnessRun } from "@/lib/harness";
import {
  buildRows,
  foldFindText,
  foldLabel,
  foldMatches,
  foldOpens,
  foldRows,
  rowItems,
  type ViewRow,
  type WorkFold,
} from "@/components/harness/timeline/rows";
import { includeId, initialFrom, windowStart } from "@/components/harness/timeline/rowWindow";
import { remarkFenceSplit, remarkWidgets } from "@/components/markdown/openui/remarkWidgets";

/** A matrix case as the replay wrote it (`write_timeline_fixtures` in
 *  `harness/testkit/matrix.rs`): the bridge's envelopes, the rows a reload
 *  reads, and the runs beside them. */
interface Case {
  envelopes: HarnessEnvelope[];
  reopened: HarnessItem[];
  runs: RunRow[];
  restarted: boolean;
}

const load = (id: string): Case =>
  JSON.parse(readFileSync(new URL(`../../fixtures/turn-model/${id}.json`, import.meta.url), "utf8"));

const THREAD = 1;

beforeEach(() => {
  useHarnessStore.getState().flushLive();
  useHarnessStore.setState({ threads: [], items: { [THREAD]: [] }, runs: {}, live: {}, holds: { [THREAD]: 1 }, queued: {}, contextDrift: {} });
});

/** Rows the live store folds from the stream, up to `upTo` envelopes. Each
 *  case's sequence numbers are its own clock, so they are dropped. */
function liveRows(c: Case, upTo = c.envelopes.length): { rows: ViewRow[]; items: HarnessItem[]; runs: Record<number, HarnessRun> } {
  for (const { sequence: _, ...env } of c.envelopes.slice(0, upTo)) {
    useHarnessStore.getState().apply(env as HarnessEnvelope);
  }
  useHarnessStore.getState().flushLive();
  const s = useHarnessStore.getState();
  const items = s.items[THREAD];
  const runs = s.runs[THREAD] ?? {};
  const streaming = !!s.live[THREAD]?.streaming;
  return { rows: buildRows(items, s.live[THREAD]?.running ?? false, { runs, streaming }), items, runs };
}

function reloadRows(c: Case): ViewRow[] {
  const runs = Object.fromEntries(c.runs.map((r) => [r.id, runFromRow(r)]));
  return buildRows(c.reopened, false, { runs });
}

const works = (rows: ViewRow[]) => rows.filter((r): r is WorkFold => r.kind === "work");
const replies = (rows: ViewRow[]) =>
  rows.filter((r) => r.kind === "item" && r.item.kind === "assistant") as Extract<ViewRow, { kind: "item" }>[];

describe("one answer outside the fold", () => {
  for (const id of [
    "claude-144-duplicate-a",
    "claude-144-duplicate-b",
    "answer-before-trailing-tool",
    "short-terminal-message",
    "claude-158-gap",
    "claude-159-recovery",
    "claude-160-gap",
  ]) {
    test(id, () => {
      const c = load(id);
      const answer = c.runs[0].answer_item_id;
      for (const rows of [liveRows(c).rows, reloadRows(c)]) {
        const shown = replies(rows);
        expect(shown.map((r) => r.item.id)).toEqual([answer!]);
        expect(shown[0].role).toBe("answer");
        // Every other reply and every tool is in the one fold, above the answer.
        const all = c.reopened.filter((i) => i.kind === "assistant" || i.kind === "tool");
        const folds = works(rows);
        const folded = folds.flatMap((w) => w.items.map((i) => i.id));
        expect(folded.sort()).toEqual(all.filter((i) => i.id !== answer).map((i) => i.id).sort());
        expect(folds.length).toBe(folded.length ? 1 : 0);
        if (folds.length) expect(rows.indexOf(folds[0])).toBeLessThan(rows.indexOf(shown[0]));
        // The question leads, and the notice (if any) stays outside.
        expect(rows[0]).toMatchObject({ kind: "item", item: { kind: "user" } });
        const notices = c.reopened.filter((i) => i.kind === "delivery_notice").length;
        expect(rows.filter((r) => r.kind === "item" && r.item.kind === "delivery_notice").length).toBe(notices);
      }
    });
  }

  test("the answer's Copy takes the answer alone", () => {
    const c = load("claude-144-duplicate-a");
    const [answer] = replies(reloadRows(c));
    expect(answer.item.content).toBe(c.reopened.find((i) => i.id === c.runs[0].answer_item_id)!.content);
  });
});

describe("artifacts outside the fold", () => {
  const c = load("artifact-source-before-tool");
  const card = c.reopened.find((i) => i.content?.includes("```openui"))!;

  test("a card written before a tool call is drawn once, after the fold", () => {
    for (const rows of [liveRows(c).rows, reloadRows(c)]) {
      const [fold] = works(rows);
      expect(fold.items.map((i) => i.id)).toContain(card.id);
      const pinned = rows.filter((r) => r.kind === "artifacts");
      expect(pinned.map((r) => r.kind === "artifacts" && [r.id, r.item.id, r.item.content])).toEqual([[`a-${card.id}`, card.id, card.content]]);
      expect(rows.indexOf(pinned[0])).toBe(rows.indexOf(fold) + 1);
      expect(rows.some((r) => r.kind === "item" && r.item.id === card.id)).toBe(false);
    }
  });

  // The fold and the pinned row parse the same reply, so a fence keeps the
  // index the whole reply gives it: `widgetKey(thread, row, index)` holds.
  const draw = (text: string, split: "artifacts" | "prose") =>
    renderToStaticMarkup(
      createElement(ReactMarkdown, {
        remarkPlugins: [remarkWidgets, [remarkFenceSplit, split]],
        components: {
          code: (p: Record<string, unknown>) => createElement("i", { "data-index": p["data-widget-index"], "data-lang": p.className }),
          "widget-pointer": (p: Record<string, unknown>) => createElement("b", { "data-lang": p["data-lang"] }),
        } as never,
        children: text,
      }),
    );
  const mixed = "Before\n\n```python\nx = 1\n```\n\n- item\n\n  ```widget\n  export default () => null\n  ```\n\n```openui\nroot = Stack([])\n```\n\nAfter";

  test("artifacts keep the whole reply's fence numbering", () => {
    const html = draw(mixed, "artifacts");
    expect(html).not.toContain("Before");
    expect(html).not.toContain("language-python");
    expect([...html.matchAll(/data-index="(\d)"/g)].map((m) => m[1])).toEqual(["0", "1"]);
    expect(draw(card.content!, "artifacts").match(/data-index/g)?.length).toBe(2);
  });

  test("the folded prose points to them and draws no fence", () => {
    const html = draw(mixed, "prose");
    expect(html).toContain("Before");
    expect(html).toContain("After");
    expect(html).toContain("language-python");
    expect(html).not.toContain("data-index=\"0\"");
    expect(html.match(/<b data-lang/g)?.length).toBe(2);
  });

  test("an unknown fence stays in the prose, as source", () => {
    const html = draw("```openuix\nnot a widget\n```", "prose");
    expect(html).toContain("openuix");
    expect(html).not.toContain("<b");
  });
});

describe("states", () => {
  test("a plain one-message reply has no fold", () => {
    const c = load("durable-command-and-answer-reference");
    for (const rows of [liveRows(c).rows, reloadRows(c)]) {
      expect(rows.map((r) => r.kind)).toEqual(["item", "item"]);
      expect(replies(rows)[0].role).toBe("answer");
    }
  });

  test("a stopped run keeps its partial reply and its stop", () => {
    for (const id of ["claude-interrupt", "codex-interrupt"]) {
      const rows = liveRows(load(id)).rows;
      expect(works(rows)).toEqual([]);
      expect(replies(rows)[0].role).toBe("partial");
      expect(rows.at(-1)).toMatchObject({ kind: "item", item: { kind: "interrupted" } });
    }
  });

  test("stopped and failed runs start expanded; a completed one starts folded", () => {
    const failed = works(liveRows(load("runtime-exit-during-stream")).rows)[0];
    expect(failed.state).toBe("failed");
    expect(foldOpens(failed.state)).toBe(true);
    const restarted = load("restart-during-stream");
    const stopped = works(reloadRows(restarted))[0];
    expect(stopped.state).toBe("interrupted");
    expect(foldOpens(stopped.state)).toBe(true);
    // The early reply has no role (startup recovery wrote none); the run's
    // answer still decides.
    expect(stopped.items.some((i) => i.kind === "assistant" && i.display_role == null)).toBe(true);
    expect(foldOpens(works(reloadRows(load("claude-144-duplicate-a")))[0].state)).toBe(false);
  });

  test("a failed run's error and a restart's notice stay outside", () => {
    const failed = liveRows(load("runtime-exit-during-stream")).rows;
    expect(failed.at(-1)).toMatchObject({ kind: "item", item: { kind: "error" } });
    expect(replies(failed)[0].role).toBe("partial");
    const restarted = reloadRows(load("restart-during-stream"));
    expect(restarted.at(-1)).toMatchObject({ kind: "item", item: { kind: "delivery_notice" } });
  });

  test("a compaction-only run shows its divider and no answer", () => {
    const rows = liveRows(load("compaction-only-mutation")).rows;
    expect(replies(rows)).toEqual([]);
    expect(rows.filter((r) => r.kind === "item" && r.item.kind === "compaction").length).toBe(1);
  });

  test("a running run folds its work open with the latest reply as the provisional answer", () => {
    const c = load("claude-144-duplicate-a");
    const tool = c.envelopes.findIndex((e) => e.event.type === "tool_started");
    const { rows } = liveRows(c, tool + 1);
    const [fold] = works(rows);
    expect(fold.state).toBe("running");
    expect(fold.endedMs).toBeNull();
    expect(foldOpens(fold.state)).toBe(true);
    const [shown] = replies(rows);
    expect(shown.role).toBe("provisional");
    expect(fold.items.map((i) => i.kind)).toEqual(["tool"]);
  });

  test("while text streams, it is the provisional answer and earlier replies are work", () => {
    const items = [
      { id: 1, thread_id: THREAD, kind: "user", run_id: 9 },
      { id: 2, thread_id: THREAD, kind: "assistant", content: "Looking.", run_id: 9 },
      { id: 3, thread_id: THREAD, kind: "tool", content: "ls", run_id: 9 },
    ] as HarnessItem[];
    const rows = buildRows(items, true, { streaming: true });
    expect(replies(rows)).toEqual([]);
    expect(works(rows)[0].items.map((i) => i.id)).toEqual([2, 3]);
  });

  test("settling replaces the live clock with the stored interval", () => {
    const c = load("runtime-exit-during-stream");
    const { runs } = liveRows(c);
    const stored = runFromRow(c.runs[0]);
    expect(runs[1]).toEqual(stored);
    expect(foldLabel({ state: "failed", startedMs: stored.started_ms, endedMs: stored.ended_ms })).toMatch(/^Failed after \d+s$/);
    expect(foldLabel({ state: "completed", startedMs: 0, endedMs: 65_000 })).toBe("Worked for 1m 5s");
    expect(foldLabel({ state: "interrupted", startedMs: 0, endedMs: 400 })).toBe("You stopped after 1s");
    expect(foldLabel({ state: "completed", startedMs: null, endedMs: 5 })).toBe("Work details");
  });

  test("rows with no run keep the step bundles", () => {
    const c = load("claude-144-duplicate-a");
    const legacy = c.reopened.map((i) => ({ ...i, run_id: null, display_role: null }));
    const rows = buildRows(legacy, false, {});
    expect(works(rows)).toEqual([]);
    expect(replies(rows).length).toBe(2);
  });

  test("Retry asks a run's own question; a turn the agent started answers none", () => {
    const c = load("claude-144-duplicate-a");
    const question = c.reopened.find((i) => i.kind === "user")!;
    expect(replies(reloadRows(c)).map((r) => r.asked)).toEqual([question]);
    const legacy = [
      { id: 1, thread_id: THREAD, kind: "user", content: "q", run_id: null },
      { id: 2, thread_id: THREAD, kind: "assistant", content: "a", run_id: null },
      { id: 3, thread_id: THREAD, kind: "unprompted", run_id: null },
      { id: 4, thread_id: THREAD, kind: "assistant", content: "b", run_id: null },
    ] as HarnessItem[];
    expect(replies(buildRows(legacy, false)).map((r) => r.asked?.id)).toEqual([1, undefined]);
  });

  test("an unsettled run nobody is running is not guessed at", () => {
    const c = load("claude-144-duplicate-a");
    const rows = buildRows(c.reopened, false, {});
    expect(works(rows)).toEqual([]);
  });
});

describe("folded rows still count", () => {
  test("the row window and a jump reach a folded reply", () => {
    const c = load("claude-144-duplicate-a");
    const rows = reloadRows(c);
    const commentary = c.reopened.find((i) => i.display_role === "commentary")!;
    const [fold] = works(rows);
    expect(rowItems(fold).map((i) => i.id)).toContain(commentary.id);
    // Pad the thread so the window starts after the fold, then jump back to it.
    const pad: ViewRow[] = Array.from({ length: 40 }, (_, i) => ({
      kind: "item",
      dim: false,
      item: { id: 100 + i, thread_id: THREAD, kind: "user" } as HarnessItem,
    }));
    const all = [...rows, ...pad];
    const from = initialFrom(all);
    expect(windowStart(all, from)).toBeGreaterThan(rows.indexOf(fold));
    expect(windowStart(all, includeId(all, from, commentary.id))).toBe(rows.indexOf(fold));
  });

  test("find opens a fold for its replies' prose, never their fences or tool rows", () => {
    const items = [
      { id: 2, kind: "assistant", content: "I'll check **the** notes.\n\n```openui\nroot = Text(\"secret\")\n```" },
      { id: 3, kind: "tool", content: "grep needle" },
    ] as HarnessItem[];
    const text = foldFindText(items);
    expect(foldMatches(text, "check the  NOTES")).toBe(true);
    expect(foldMatches(text, "secret")).toBe(false);
    expect(foldMatches(text, "needle")).toBe(false);
    expect(foldMatches(text, "  ")).toBe(false);
  });

  test("inside a fold, consecutive tools bundle except the step still running", () => {
    const items = [
      { id: 2, kind: "tool" },
      { id: 3, kind: "tool" },
      { id: 4, kind: "assistant", content: "Halfway." },
      { id: 5, kind: "tool" },
      { id: 6, kind: "thinking" },
    ] as HarnessItem[];
    expect(foldRows(items).map((r) => r.kind)).toEqual(["bundle", "item", "bundle"]);
    expect(foldRows(items, true).map((r) => r.kind)).toEqual(["bundle", "item", "item", "item"]);
  });
});

describe("the stream never rebuilds history", () => {
  test("a delta leaves rows, runs and other tools' output untouched", () => {
    const c = load("claude-144-duplicate-a");
    const tool = c.envelopes.findIndex((e) => e.event.type === "tool_started");
    liveRows(c, tool + 1);
    const before = useHarnessStore.getState();
    const base = { threadId: THREAD, provider: "claude", itemId: null } as const;
    useHarnessStore.getState().apply({ ...base, event: { type: "tool_output_delta", id: "a", text: "1" } } as HarnessEnvelope);
    useHarnessStore.getState().apply({ ...base, event: { type: "assistant_delta", text: "x" } } as HarnessEnvelope);
    useHarnessStore.getState().flushLive();
    const mid = useHarnessStore.getState();
    useHarnessStore.getState().apply({ ...base, event: { type: "tool_output_delta", id: "b", text: "2" } } as HarnessEnvelope);
    useHarnessStore.getState().flushLive();
    const after = useHarnessStore.getState();
    expect(after.items).toBe(before.items);
    expect(after.runs).toBe(before.runs);
    expect(after.live[THREAD].toolOutput.a).toBe(mid.live[THREAD].toolOutput.a);
    expect(after.live[THREAD].streaming).toBe("x");
  });
});

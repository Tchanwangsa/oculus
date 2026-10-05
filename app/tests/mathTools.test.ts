import { beforeEach, describe, expect, test } from "bun:test";
import { ChangeSet, EditorState } from "@codemirror/state";

const store = new Map<string, string>();
(globalThis as { localStorage?: unknown }).localStorage = {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => void store.set(k, v),
  removeItem: (k: string) => void store.delete(k),
};

const { mathTools, mathToolsOpen, nextOpen, openMathTools } = await import(
  "../src/components/documents/editor/mathTools"
);
const { noteMarkdown } = await import("../src/components/documents/editor/language");
const { QUICK_PICKS, popularEntries, quickPicks, recordUse } = await import(
  "../src/components/documents/editor/mathUsage"
);

describe("maths toolbox open state", () => {
  test("opens only on request, on the caret's maths", () => {
    expect(nextOpen(null, null, undefined, 4)).toBeNull();
    expect(nextOpen(null, null, "quick", 4)).toEqual({ nodeFrom: 4, kind: "quick" });
    expect(nextOpen({ nodeFrom: 4, kind: "quick" }, null, "full", 4)).toEqual({ nodeFrom: 4, kind: "full" });
    expect(nextOpen({ nodeFrom: 4, kind: "full" }, null, null, 4)).toBeNull();
    // No maths at the caret: nothing to open on.
    expect(nextOpen(null, null, "full", null)).toBeNull();
  });

  test("maps through edits and closes when the caret's maths changes", () => {
    const open = { nodeFrom: 4, kind: "full" as const };
    const typedBefore = ChangeSet.of({ from: 0, insert: "ab" }, 20);
    expect(nextOpen(open, typedBefore, undefined, 6)).toEqual({ nodeFrom: 6, kind: "full" });
    expect(nextOpen(open, null, undefined, 12)).toBeNull();
    expect(nextOpen(open, null, undefined, null)).toBeNull();
  });

  test("in a note: hidden by default, open until the caret leaves that maths", () => {
    const doc = "Text $x^2$ and $y$.";
    let state = EditorState.create({ doc, extensions: [noteMarkdown(), mathTools()], selection: { anchor: 7 } });
    expect(mathToolsOpen(state)).toBeNull();
    state = state.update({ effects: openMathTools.of("full") }).state;
    expect(mathToolsOpen(state)).toBe("full");
    // Typing inside the maths and before it keeps it open.
    state = state.update({ changes: { from: 7, insert: "+1" }, selection: { anchor: 9 } }).state;
    expect(mathToolsOpen(state)).toBe("full");
    state = state.update({ changes: { from: 0, insert: "More. " }, selection: { anchor: 15 } }).state;
    expect(mathToolsOpen(state)).toBe("full");
    // Into the other maths: closed, and it stays closed coming back.
    const other = state.doc.toString().indexOf("$y$") + 1;
    state = state.update({ selection: { anchor: other } }).state;
    expect(mathToolsOpen(state)).toBeNull();
    state = state.update({ selection: { anchor: 15 } }).state;
    expect(mathToolsOpen(state)).toBeNull();
    // Opening outside maths does nothing.
    state = state.update({ selection: { anchor: 0 }, effects: openMathTools.of("full") }).state;
    expect(mathToolsOpen(state)).toBeNull();
  });
});

describe("quick picks", () => {
  beforeEach(() => store.clear());

  test("are the Popular tab's first five before the subject has history", () => {
    expect(quickPicks(1)).toEqual(popularEntries().slice(0, QUICK_PICKS));
    expect(quickPicks(1)).toHaveLength(QUICK_PICKS);
  });

  test("lead with the entry last used in the subject", () => {
    const [often, last] = [popularEntries()[10], popularEntries()[11]];
    recordUse(often, 1);
    recordUse(often, 1);
    recordUse(last, 1);
    expect(quickPicks(1).slice(0, 2).map((e) => e.template)).toEqual([last.template, often.template]);
    expect(quickPicks(1)).toHaveLength(QUICK_PICKS);
    expect(quickPicks(2)[0].template).not.toBe(last.template);
  });
});

import { describe, expect, test } from "bun:test";
import { ChangeSet, EditorState, type TransactionSpec } from "@codemirror/state";

const store = new Map<string, string>();
(globalThis as { localStorage?: unknown }).localStorage = {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => void store.set(k, v),
  removeItem: (k: string) => void store.delete(k),
};

const { mathTools, mathToolsOpen, nextOpen, openMathTools } = await import(
  "@/components/documents/editor/math/tools/mathTools"
);
const { noteMarkdown } = await import("@/components/documents/editor/core/language");
const { mathAt } = await import("@/components/documents/editor/math/mathContext");
const { fieldKey } = await import("@/components/documents/editor/math/tools/mathTools/keys");
const { fieldTools } = await import("@/components/documents/editor/math/field/mathField");

describe("maths toolbox open state", () => {
  test("opens only on request, on the caret's maths", () => {
    expect(nextOpen(null, null, undefined, 4)).toBeNull();
    expect(nextOpen(null, null, "full", 4)).toEqual({ nodeFrom: 4, kind: "full" });
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

describe("empty inline pairs", () => {
  const at = (doc: string, pos: number) =>
    mathAt(EditorState.create({ doc, extensions: [noteMarkdown()], selection: { anchor: pos } }), pos);

  test("`$$` mid-line and `\\(\\)` anywhere are empty inline maths", () => {
    for (const [doc, pos, node] of [["a $$ b", 3, false], ["\\(\\)", 2, true], ["- \\(\\) b", 4, true]] as const) {
      const ctx = at(doc, pos);
      expect(ctx?.node != null).toBe(node);
      expect(ctx?.display).toBe(false);
      expect([ctx?.from, ctx?.to]).toEqual([pos, pos]);
    }
  });

  test("a lone `$$` line opens a block, not inline maths", () => {
    expect(at("Hello\n\n$$", 8)).toBeNull();
    expect(at("$$\n\n$$", 3)?.display).toBe(true);
  });
});

describe("Space in the visual field", () => {
  /** A focused note with the caret in `$x^2$`, as an `EditorView` stands in for it. */
  const note = () => {
    const view = {
      state: EditorState.create({ doc: "Text $x^2$ and", extensions: [noteMarkdown(), mathTools()], selection: { anchor: 7 } }),
      dispatch(spec: TransactionSpec) {
        view.state = view.state.update(spec).state;
      },
    };
    return view;
  };
  const space = { key: " ", code: "Space", shiftKey: false, metaKey: false, ctrlKey: false, altKey: false } as KeyboardEvent;
  const field = (mode: "math" | "command", free = true) => {
    const opened: string[] = [];
    return {
      opened,
      field: {
        mode: () => mode,
        spaceFree: () => free,
        isEmpty: () => false,
        display: false,
        openList: () => opened.push("list"),
      } as never,
    };
  };

  test("opens the field's picks where Space is free", () => {
    const view = note();
    const f = field("math");
    expect(fieldKey(view as never, space, f.field)).toBe(true);
    expect(f.opened).toEqual(["list"]);
    const busy = field("math", false);
    expect(fieldKey(view as never, space, busy.field)).toBe(false);
    expect(busy.opened).toEqual([]);
  });

  test("leaves Space to a \\command being typed and to an open toolbox", () => {
    const view = note();
    const typing = field("command");
    expect(fieldKey(view as never, space, typing.field)).toBe(false);
    view.dispatch({ effects: openMathTools.of("full") });
    const f = field("math");
    expect(fieldKey(view as never, space, f.field)).toBe(false);
    expect(f.opened).toEqual([]);
  });

  test("Space again in the picks opens the full toolbox (`fieldTools`)", () => {
    const view = note();
    expect(mathToolsOpen(view.state)).toBeNull();
    for (const open of view.state.facet(fieldTools)) open(view as never);
    expect(mathToolsOpen(view.state)).toBe("full");
  });
});

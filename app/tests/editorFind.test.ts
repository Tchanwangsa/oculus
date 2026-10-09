import { describe, expect, test } from "bun:test";
import { history, undo, undoDepth } from "@codemirror/commands";
import { EditorSelection, EditorState, type Extension, type TransactionSpec } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import {
  FIND_CAP,
  clearFind,
  findExtension,
  findRevealed,
  findStatus,
  findStep,
  replaceAll,
  replaceCurrent,
  selectionQuery,
  setFindQuery,
} from "../src/components/documents/editor/find";

/** Just enough of a view for the commands, which only read state and dispatch. */
function fakeView(doc: string, extra: Extension = [], anchor = 0) {
  const view = {
    state: EditorState.create({ doc, selection: { anchor }, extensions: [history(), findExtension(), extra] }),
    dispatched: 0,
    dispatch(spec: TransactionSpec) {
      view.state = view.state.update(spec).state;
      view.dispatched++;
    },
  };
  return view as typeof view & EditorView;
}

const sel = (v: EditorView) => [v.state.selection.main.from, v.state.selection.main.to];

describe("matching", () => {
  test("is literal and case-insensitive by default", () => {
    const v = fakeView("Cat cat CAT c.t");
    setFindQuery(v, "cat");
    expect(findStatus(v.state)).toEqual({ total: 3, current: 1, capped: false });
    setFindQuery(v, "c.t");
    expect(findStatus(v.state).total).toBe(1);
  });

  test("case-sensitive when asked", () => {
    const v = fakeView("Cat cat CAT");
    setFindQuery(v, "cat", { caseSensitive: true });
    expect(findStatus(v.state)).toEqual({ total: 1, current: 1, capped: false });
    expect(sel(v)).toEqual([4, 7]);
  });

  test("typing selects the first match from the selection's start", () => {
    const v = fakeView("ab ab ab", [], 1);
    setFindQuery(v, "a");
    expect(sel(v)).toEqual([3, 4]);
    setFindQuery(v, "ab");
    expect(sel(v)).toEqual([3, 5]);
    expect(findStatus(v.state).current).toBe(2);
    expect(findRevealed(v.state)).toBe(true);
  });

  test("caps the count", () => {
    const v = fakeView("x".repeat(FIND_CAP + 5));
    setFindQuery(v, "x");
    expect(findStatus(v.state)).toEqual({ total: FIND_CAP, current: 1, capped: true });
  });

  test("an edit maps the current match, and drops it once it stops matching", () => {
    const v = fakeView("one foo two foo");
    setFindQuery(v, "foo");
    findStep(v, false);
    expect(sel(v)).toEqual([12, 15]);
    v.dispatch({ changes: { from: 0, insert: ">> " } });
    expect(findStatus(v.state)).toEqual({ total: 2, current: 2, capped: false });
    v.dispatch({ changes: { from: 16, to: 17, insert: "x" } });
    expect(findStatus(v.state)).toEqual({ total: 1, current: 0, capped: false });
  });

  test("status is the same object until it changes", () => {
    const v = fakeView("a a");
    setFindQuery(v, "a");
    const s = findStatus(v.state);
    expect(findStatus(v.state)).toBe(s);
    findStep(v, false);
    expect(findStatus(v.state)).not.toBe(s);
  });
});

describe("stepping", () => {
  test("wraps both ways", () => {
    const v = fakeView("a1 a2 a3");
    setFindQuery(v, "a");
    expect(sel(v)).toEqual([0, 1]);
    findStep(v, false);
    findStep(v, false);
    expect(sel(v)).toEqual([6, 7]);
    findStep(v, false);
    expect(sel(v)).toEqual([0, 1]);
    findStep(v, true);
    expect(sel(v)).toEqual([6, 7]);
    expect(findStatus(v.state).current).toBe(3);
    findStep(v, true);
    expect(sel(v)).toEqual([3, 4]);
  });

  test("goes from the caret", () => {
    const v = fakeView("a1 a2 a3");
    setFindQuery(v, "a");
    v.dispatch({ selection: { anchor: 4 } });
    expect(findRevealed(v.state)).toBe(false);
    findStep(v, false);
    expect(sel(v)).toEqual([6, 7]);
    v.dispatch({ selection: { anchor: 3 } });
    findStep(v, true);
    expect(sel(v)).toEqual([0, 1]);
  });

  test("nothing to find is a no-op", () => {
    const v = fakeView("abc");
    setFindQuery(v, "z");
    expect(findStatus(v.state)).toEqual({ total: 0, current: 0, capped: false });
    expect(findStep(v, false)).toBe(false);
    expect(sel(v)).toEqual([0, 0]);
  });

  test("clearFind drops the query and keeps the selection", () => {
    const v = fakeView("a b a");
    setFindQuery(v, "b");
    clearFind(v);
    expect(findStatus(v.state)).toEqual({ total: 0, current: 0, capped: false });
    expect(sel(v)).toEqual([2, 3]);
    expect(findRevealed(v.state)).toBe(false);
  });
});

describe("replacing", () => {
  test("replaceCurrent replaces the selected match and moves on", () => {
    const v = fakeView("cat cat cat");
    setFindQuery(v, "cat");
    expect(replaceCurrent(v, "dog")).toBe(true);
    expect(v.state.doc.toString()).toBe("dog cat cat");
    expect(sel(v)).toEqual([4, 7]);
    expect(findStatus(v.state)).toEqual({ total: 2, current: 1, capped: false });
  });

  test("replaceCurrent with the selection off a match only selects the next", () => {
    const v = fakeView("cat cat");
    setFindQuery(v, "cat");
    v.dispatch({ selection: { anchor: 2 } });
    replaceCurrent(v, "dog");
    expect(v.state.doc.toString()).toBe("cat cat");
    expect(sel(v)).toEqual([4, 7]);
  });

  test("a replacement holding the query doesn't loop", () => {
    const v = fakeView("a b a");
    setFindQuery(v, "a");
    replaceCurrent(v, "aa");
    expect(v.state.doc.toString()).toBe("aa b a");
    expect(sel(v)).toEqual([5, 6]);
  });

  test("replaceAll is one undo step, past the cap too", () => {
    const doc = "x ".repeat(FIND_CAP + 3);
    const v = fakeView(doc);
    setFindQuery(v, "X");
    const before = v.dispatched;
    expect(replaceAll(v, "y")).toBe(FIND_CAP + 3);
    expect(v.dispatched - before).toBe(1);
    expect(v.state.doc.toString()).toBe("y ".repeat(FIND_CAP + 3));
    expect(undoDepth(v.state)).toBe(1);
    undo(v);
    expect(v.state.doc.toString()).toBe(doc);
  });

  test("replacements are tagged and undoable", () => {
    const v = fakeView("cat cat");
    setFindQuery(v, "cat");
    let event = "";
    const dispatch = v.dispatch.bind(v);
    v.dispatch = ((spec: TransactionSpec) => {
      if (spec.userEvent) event ||= spec.userEvent;
      dispatch(spec);
    }) as typeof v.dispatch;
    replaceCurrent(v, "dog");
    expect(event).toBe("input.replace");
    undo(v);
    expect(v.state.doc.toString()).toBe("cat cat");
  });

  test("read-only replaces nothing", () => {
    const v = fakeView("cat cat", EditorState.readOnly.of(true));
    setFindQuery(v, "cat");
    expect(replaceCurrent(v, "dog")).toBe(false);
    expect(v.state.doc.toString()).toBe("cat cat");
    expect(replaceAll(v, "dog")).toBe(0);
    expect(v.state.doc.toString()).toBe("cat cat");
  });
});

describe("selectionQuery", () => {
  const at = (doc: string, from: number, to: number) =>
    selectionQuery(EditorState.create({ doc, selection: EditorSelection.single(from, to) }));

  test("seeds from a one-line selection only", () => {
    expect(at("hello world", 0, 5)).toBe("hello");
    expect(at("hello\nworld", 3, 8)).toBe("");
    expect(at("hello", 2, 2)).toBe("");
  });
});

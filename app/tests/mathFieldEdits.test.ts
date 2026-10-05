import { describe, expect, test } from "bun:test";
import { history, undo, redo, undoDepth } from "@codemirror/commands";
import { EditorSelection, EditorState, Transaction, type TransactionSpec } from "@codemirror/state";

import {
  FIELD_INPUT,
  fieldWrite,
  caretAfterChange,
  minimalChange,
  selectionPastField,
  writableSpan,
  type FieldSpan,
} from "../src/components/documents/editor/mathFieldEdits";

const MATRIX =
  "W _ { 1 } = R _ { G } Z _ { a } = \\left[ \\begin{array} { c c } { { \\cos \\theta } } & { { - \\sin \\theta } } \\\\ { { \\sin \\theta } } & { { \\cos \\theta } } \\end{array} \\right] .";
const DOC = ["A paragraph above.", "", "$$", MATRIX, "$$", "", "", "After the block."].join("\n");

/** The block's span in `doc`, as the visual state would hold it. */
function blockSpan(doc: string, id = 1): FieldSpan {
  const start = doc.indexOf("$$");
  const close = doc.indexOf("$$", start + 2);
  return { id, start, end: close + 2, from: start + 2, to: close, block: true };
}

/** One flush from the field: the LaTeX between the delimiters becomes `latex`. */
function fieldTyped(state: EditorState, span: FieldSpan, latex: string, time: number): TransactionSpec {
  const current = state.sliceDoc(span.from, span.to);
  const change = minimalChange(current, `\n${latex}\n`, span.from)!;
  return {
    changes: change,
    selection: { anchor: span.from },
    userEvent: FIELD_INPUT,
    annotations: [fieldWrite.of(span.id), Transaction.time.of(time)],
  };
}

function apply(state: EditorState, spec: TransactionSpec): EditorState {
  return state.update(spec).state;
}

function run(state: EditorState, command: typeof undo): EditorState {
  let next = state;
  command({ state, dispatch: (tr) => (next = tr.state) });
  return next;
}

describe("minimalChange", () => {
  test("covers only what differs", () => {
    expect(minimalChange("x+y", "x+yz", 10)).toEqual({ from: 13, to: 13, insert: "z" });
    expect(minimalChange("x+yz", "x+y", 10)).toEqual({ from: 13, to: 14, insert: "" });
    expect(minimalChange("a\\alpha b", "a\\beta b", 0)).toEqual({ from: 2, to: 6, insert: "bet" });
    expect(minimalChange("same", "same", 0)).toBeNull();
  });

  test("doesn't split a surrogate pair", () => {
    const change = minimalChange("x\u{1D400}", "x\u{1D401}", 0)!;
    expect(change.from).toBe(1);
    expect(change.insert).toBe("\u{1D401}");
  });
});

describe("writableSpan", () => {
  const state = EditorState.create({ doc: DOC });
  const span = blockSpan(DOC, 7);

  test("the field's own maths, unchanged since it last looked", () => {
    expect(writableSpan(state, span, 7, MATRIX)).toBe(span);
  });

  test("another maths, or none, takes no write", () => {
    expect(writableSpan(state, { ...span, id: 8 }, 7, MATRIX)).toBeNull();
    expect(writableSpan(state, null, 7, MATRIX)).toBeNull();
  });

  test("maths changed from outside takes no write", () => {
    expect(writableSpan(state, span, 7, "x")).toBeNull();
  });
});

describe("selectionPastField", () => {
  const state = EditorState.create({ doc: DOC });
  const span = blockSpan(DOC);
  const lastLineEnd = state.doc.lineAt(span.end).to;
  const firstLineStart = state.doc.lineAt(span.start).from;
  const after = state.doc.line(7).from;

  test("extending down from inside the maths starts after its last line", () => {
    const sel = selectionPastField(state.doc, span, EditorSelection.single(span.from, after))!;
    expect(sel.main.anchor).toBe(lastLineEnd);
    expect(sel.main.head).toBe(after);
  });

  test("extending up starts before its first line", () => {
    const sel = selectionPastField(state.doc, span, EditorSelection.single(span.from + 5, 3))!;
    expect(sel.main.anchor).toBe(firstLineStart);
    expect(sel.main.head).toBe(3);
  });

  test("leaves selections that stay inside, or start outside, alone", () => {
    expect(selectionPastField(state.doc, span, EditorSelection.single(span.from, span.to))).toBeNull();
    expect(selectionPastField(state.doc, span, EditorSelection.single(lastLineEnd, after))).toBeNull();
    expect(selectionPastField(state.doc, span, EditorSelection.single(0, after))).toBeNull();
  });

  test("inline maths uses its delimiters as edges", () => {
    const doc = EditorState.create({ doc: "see $x+y$ here" }).doc;
    const inline = { start: 4, end: 9, block: false };
    expect(selectionPastField(doc, inline, EditorSelection.single(6, 14))!.main.anchor).toBe(9);
    expect(selectionPastField(doc, inline, EditorSelection.single(6, 0))!.main.anchor).toBe(4);
  });
});

describe("the field's writes in the note's history", () => {
  const base = () => EditorState.create({ doc: DOC, extensions: history() });

  test("a run of keystrokes is one undo step, and undo restores the source exactly", () => {
    let state = base();
    let span = blockSpan(state.doc.toString());
    // The first edit re-serialises the whole LaTeX; later ones touch a character.
    for (const [i, latex] of ["W_1=x", "W_1=xy", "W_1=xyz"].entries()) {
      state = apply(state, fieldTyped(state, span, latex, 1000 + i * 100));
      span = blockSpan(state.doc.toString());
    }
    expect(state.sliceDoc(span.from, span.to)).toBe("\nW_1=xyz\n");
    expect(undoDepth(state)).toBe(1);
    state = run(state, undo);
    expect(state.doc.toString()).toBe(DOC);
    state = run(state, redo);
    expect(state.sliceDoc(blockSpan(state.doc.toString()).from, blockSpan(state.doc.toString()).to)).toBe("\nW_1=xyz\n");
  });

  test("a pause starts a new step", () => {
    let state = base();
    let span = blockSpan(DOC);
    state = apply(state, fieldTyped(state, span, "a", 1000));
    span = blockSpan(state.doc.toString());
    state = apply(state, fieldTyped(state, span, "ab", 3000));
    expect(undoDepth(state)).toBe(2);
    state = run(state, undo);
    expect(state.sliceDoc(span.from, span.to)).toBe("\na\n");
  });

  test("undo after leaving the field restores the maths", () => {
    let state = base();
    const span = blockSpan(DOC);
    state = apply(state, { selection: { anchor: span.from + 3 } });
    state = apply(state, fieldTyped(state, span, "z", 1000));
    // Esc: the caret back in the note below the block.
    state = apply(state, { selection: { anchor: state.doc.line(7).from }, userEvent: "select" });
    state = run(state, undo);
    expect(state.doc.toString()).toBe(DOC);
    // Back in the maths, so the field reopens on it.
    expect(state.selection.main.head).toBe(span.from + 3);
  });

  test("Delete on a selection extended from the open field removes only what it highlights", () => {
    let state = base();
    const span = blockSpan(DOC);
    // The note's caret sits at the start of the LaTeX while the field is open.
    state = apply(state, { selection: { anchor: span.from } });
    const blank = state.doc.line(7).from;
    const sel = selectionPastField(state.doc, span, EditorSelection.single(span.from, blank))!;
    state = apply(state, { selection: sel, userEvent: "select.pointer" });
    const { from, to } = state.selection.main;
    expect(state.sliceDoc(from, to)).toBe("\n\n");
    state = apply(state, { ...state.replaceSelection(""), userEvent: "delete.selection" });
    expect(state.doc.toString()).toBe(DOC.slice(0, from) + DOC.slice(to));
    state = run(state, undo);
    expect(state.doc.toString()).toBe(DOC);
  });
});

describe("caretAfterChange", () => {
  // Atom 0 is the field's root; offset k sits after atom k.
  const atoms = (s: string) => [..."^" + s];
  test("an undone insertion puts the caret where it was", () => {
    // "abXYZ|cd" → "ab|cd"
    expect(caretAfterChange(atoms("abXYZcd"), atoms("abcd"), 5)).toBe(2);
  });
  test("a redone insertion puts the caret after it", () => {
    expect(caretAfterChange(atoms("abcd"), atoms("abXYZcd"), 2)).toBe(5);
  });
  test("a replacement puts the caret after the new atoms", () => {
    expect(caretAfterChange(atoms("abXcd"), atoms("abYYcd"), 3)).toBe(4);
  });
  test("an undone copy pasted above its original goes back to the paste", () => {
    // "abcd|cd" → "ab|cd", not the field's end.
    expect(caretAfterChange(atoms("abcdcd"), atoms("abcd"), 4)).toBe(2);
  });
  test("an undone copy pasted below its original goes back to the paste", () => {
    // "abcdcd|" → "abcd|"
    expect(caretAfterChange(atoms("abcdcd"), atoms("abcd"), 6)).toBe(4);
  });
  test("a redone copy goes after itself", () => {
    expect(caretAfterChange(atoms("abcd"), atoms("abcdcd"), 2)).toBe(4);
  });
  test("an edit after the caret puts it at the edit", () => {
    expect(caretAfterChange(atoms("abcdX"), atoms("abcd"), 2)).toBe(4);
  });
});

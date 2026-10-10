import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { history, redo, undo, undoDepth } from "@codemirror/commands";
import { EditorState, type TransactionSpec } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import { noteMarkdown } from "@/components/documents/editor/core/language";
import { focusedField, setFocused } from "@/components/documents/editor/core/liveFocus";
import { MATH_ENGINE_KEY } from "@/components/documents/editor/math/field/fieldEngine";
import { leaveMaths, removeMaths } from "@/components/documents/editor/math/field/fieldNote";
import { readsCleanly, visualMath, visualMathField } from "@/components/documents/editor/math/field/mathField";
import {
  caretAfterEdit,
  resync,
  writableTarget,
  writeStep,
  type FieldText,
  type NoteView,
} from "@/components/documents/editor/math/field/rustField/write";
import { MathField, type FieldCommand, type Step } from "@/lib/maths";

/** The switch on for this file only: `rustField` reads it on each call. */
const previous = (globalThis as { localStorage?: unknown }).localStorage;
beforeAll(() => {
  (globalThis as { localStorage?: unknown }).localStorage = {
    getItem: (k: string) => (k === MATH_ENGINE_KEY ? "rust" : null),
  };
});
afterAll(() => {
  (globalThis as { localStorage?: unknown }).localStorage = previous;
});

/** A Live note's state, focused, as an `EditorView` stands in for it. */
class Note implements NoteView {
  state: EditorState;
  constructor(doc: string, anchor: number) {
    this.state = EditorState.create({
      doc,
      selection: { anchor },
      extensions: [noteMarkdown(), history(), focusedField, visualMathField],
    });
    this.dispatch({ effects: setFocused.of(true) });
  }
  dispatch(spec: TransactionSpec) {
    this.state = this.state.update(spec).state;
  }
  focus() {}
  get doc() {
    return this.state.doc.toString();
  }
  undo() {
    undo({ state: this.state, dispatch: (tr) => (this.state = tr.state) });
  }
  redo() {
    redo({ state: this.state, dispatch: (tr) => (this.state = tr.state) });
  }
}

/** The Rust field on the note's open maths, without its view: the edit
 *  model and what the controller keeps of the note. */
class Field {
  field: MathField;
  text: FieldText;
  constructor(readonly note: Note) {
    const v = visualMath(note.state);
    if (!v) throw new Error("no field open");
    const source = note.state.sliceDoc(v.from, v.to).trim();
    this.field = MathField.open(source, v.display);
    this.text = { id: v.id, shown: source, written: source };
  }
  run(...commands: FieldCommand[]): Step {
    let step!: Step;
    for (const c of commands) {
      step = this.field.run(c);
      this.field = step.field;
      writeStep(this.note, this.text, step);
    }
    return step;
  }
  type(keys: string): Step {
    return this.run(...[...keys].map((k) => ({ insert: k })));
  }
  /** As the widget's `updateDOM` does after an outside change. */
  sync() {
    const v = visualMath(this.note.state)!;
    const source = this.note.state.sliceDoc(v.from, v.to).trim();
    const caret = resync(this.text, source, this.field.stops[this.field.head]);
    if (caret != null) this.field = MathField.open(source, v.display).caretAt(caret);
  }
  get caret() {
    return this.field.stops[this.field.head];
  }
}

describe("the Rust field's writes", () => {
  test("the switch makes the edit model the judge of what opens", () => {
    expect(readsCleanly("\\frac{a}{b}", false)).toBe(true);
    expect(readsCleanly("\\frac{a}{", false)).toBe(false);
  });

  test("a typed key goes into the note between the delimiters", () => {
    const note = new Note("See $y$ here.", 6);
    const f = new Field(note);
    f.type("x");
    expect(note.doc).toBe("See $yx$ here.");
    expect(note.state.selection.main.head).toBe(7);
    expect(visualMath(note.state)?.id).toBe(f.text.id);
    f.type("+1");
    expect(note.doc).toBe("See $yx+1$ here.");
    expect(undoDepth(note.state)).toBe(1);
  });

  test("a shortcut's expansion is an undo step of its own", () => {
    const note = new Note("Let $a+$ go", 7);
    const f = new Field(note);
    f.type("sin");
    expect(note.doc).toBe("Let $a+\\sin$ go");
    expect(undoDepth(note.state)).toBe(2);
    note.undo();
    expect(note.doc).toBe("Let $a+sin$ go");
    note.undo();
    expect(note.doc).toBe("Let $a+$ go");
    note.redo();
    note.redo();
    expect(note.doc).toBe("Let $a+\\sin$ go");
  });

  test("undo keeps the field open and puts its caret after what changed", () => {
    const note = new Note("Let $a+b$ go", 8);
    const f = new Field(note);
    f.run({ left: {} });
    f.type("sin");
    // A space keeps the control word off the letter after it.
    expect(note.doc).toBe("Let $a+\\sin b$ go");
    note.undo();
    expect(note.doc).toBe("Let $a+sinb$ go");
    expect(visualMath(note.state)?.id).toBe(f.text.id);
    f.sync();
    expect(f.field.source).toBe("a+sinb");
    expect(f.caret).toBe(5);
    // The field's own writes are recognised: no reload for them.
    expect(resync(f.text, "a+sinb", f.caret)).toBeNull();
    f.type("x");
    expect(note.doc).toBe("Let $a+sinxb$ go");
  });

  test("a matrix edit is an undo step of its own", () => {
    const note = new Note("M $($.", 4);
    const f = new Field(note);
    f.type("a");
    const step = f.run({ insert: " " });
    expect(step.isolate).toBe(true);
    expect(note.doc).toBe("M $\\begin{pmatrix}a & \\end{pmatrix}$.");
    f.type("b");
    expect(note.doc).toBe("M $\\begin{pmatrix}a & b\\end{pmatrix}$.");
    expect(undoDepth(note.state)).toBe(3);
    note.undo();
    expect(note.doc).toBe("M $\\begin{pmatrix}a & \\end{pmatrix}$.");
    note.undo();
    expect(note.doc).toBe("M $(a$.");
  });

  test("an empty `\\(\\)` becomes `$…$` with its first key", () => {
    const note = new Note("Q \\(\\) end", 4);
    const f = new Field(note);
    expect(f.field.source).toBe("");
    f.type("x");
    expect(note.doc).toBe("Q $x$ end");
    expect(visualMath(note.state)?.id).toBe(f.text.id);
    f.type("y");
    expect(note.doc).toBe("Q $xy$ end");
  });

  test("Esc in an empty field leaves it, after the maths", () => {
    const note = new Note("Q \\(\\) end", 4);
    const f = new Field(note);
    const step = f.run("escape");
    expect(step.effect).toBe("leaveRight");
    leaveMaths(note as unknown as EditorView, writableTarget(note, f.text), "forward");
    expect(note.state.selection.main.head).toBe(6);
    expect(visualMath(note.state)).toBeNull();
  });

  test("Backspace in an empty field removes the maths", () => {
    const note = new Note("Q \\(\\) end", 4);
    const f = new Field(note);
    expect(f.run("backspace").effect).toBe("removeMaths");
    removeMaths(note as unknown as EditorView, writableTarget(note, f.text));
    expect(note.doc).toBe("Q  end");
  });

  test("a block's rows stay one per line, its edges one break each", () => {
    const note = new Note("Top\n$$\nx\n$$\nEnd", 7);
    const f = new Field(note);
    f.run("enter");
    expect(note.doc).toBe("Top\n$$\nx \\\\\n$$\nEnd");
    expect(visualMath(note.state)?.id).toBe(f.text.id);
    f.type("y");
    expect(note.doc).toBe("Top\n$$\nx \\\\\ny\n$$\nEnd");
    note.undo();
    expect(note.doc).toBe("Top\n$$\nx\n$$\nEnd");
  });

  test("an empty block's first key goes on its own line", () => {
    const note = new Note("Top\n$$\n\n$$\nEnd", 7);
    const f = new Field(note);
    f.type("z");
    expect(note.doc).toBe("Top\n$$\nz\n$$\nEnd");
  });

  test("a write is dropped once the maths changed from outside", () => {
    const note = new Note("See $y$ here.", 6);
    const f = new Field(note);
    note.dispatch({ changes: { from: 5, insert: "q" }, selection: { anchor: 7 } });
    f.type("x");
    expect(note.doc).toBe("See $qy$ here.");
  });
});

describe("caretAfterEdit", () => {
  test("is the end of what changed, the common tail stopping at the old caret", () => {
    expect(caretAfterEdit("\\sin", "sin", 4)).toBe(3);
    expect(caretAfterEdit("a+xb", "a+b", 3)).toBe(2);
    expect(caretAfterEdit("a+b", "a+xb", 2)).toBe(3);
    expect(caretAfterEdit("aa", "aaa", 2)).toBe(3);
  });
});

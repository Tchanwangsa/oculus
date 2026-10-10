import { describe, expect, test } from "bun:test";
import { history, redo, undo, undoDepth } from "@codemirror/commands";
import { EditorState, type TransactionSpec } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import { noteMarkdown } from "@/components/documents/editor/core/language";
import { focusedField, setFocused } from "@/components/documents/editor/core/liveFocus";
import { leaveMaths, newlineBeside, removeMaths } from "@/components/documents/editor/math/field/fieldNote";
import { enterSide } from "@/components/documents/editor/math/field/rustField/keys";
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
  /** Every spec dispatched, and how many measures were asked for. */
  specs: TransactionSpec[] = [];
  measures = 0;
  dispatch(spec: TransactionSpec) {
    this.specs.push(spec);
    this.state = this.state.update(spec).state;
  }
  focus() {}
  requestMeasure() {
    this.measures++;
  }
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

  test("leaving never has CodeMirror scroll: the caret is kept in view on the next measure", () => {
    for (const [doc, at, dir, head] of [
      ["Top\n\n$$\nx\n$$\n\nEnd", 8, "backward", 4],
      ["Top\n\n$$\nx\n$$\n\nEnd", 8, "forward", 13],
      ["A $x$ b", 3, "backward", 2],
    ] as const) {
      const note = new Note(doc, at);
      const f = new Field(note);
      note.specs = [];
      leaveMaths(note as unknown as EditorView, writableTarget(note, f.text), dir);
      expect(note.state.selection.main.head).toBe(head);
      expect(note.specs.some((s) => s.scrollIntoView)).toBe(false);
      expect(note.measures).toBe(1);
    }
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

describe("a display block's edges", () => {
  // "Top\n" is 4; the block's `$$` lines run 4..11; "End" starts at 12.
  const doc = "Top\n$$\nx\n$$\nEnd";

  test("a caret at either edge opens the field, as inside the block", () => {
    for (const at of [4, 11]) {
      const v = visualMath(new Note(doc, at).state);
      expect(v?.block).toBe(true);
      expect([v?.start, v?.end]).toEqual([4, 11]);
    }
  });

  test("the lines beside it stay text", () => {
    expect(visualMath(new Note(doc, 3).state)).toBeNull();
    expect(visualMath(new Note(doc, 12).state)).toBeNull();
  });

  test("a selection taking the whole block, or running across it, keeps it rendered", () => {
    for (const [anchor, head] of [[4, 11], [2, 14], [11, 4]]) {
      const note = new Note(doc, 0);
      note.dispatch({ selection: { anchor, head } });
      expect(visualMath(note.state)).toBeNull();
    }
  });

  test("leaving past the note's start or end edits nothing and keeps the field", () => {
    for (const [doc, at, dir] of [
      ["$$\nx\n$$", 3, "backward"],
      ["$$\nx\n$$", 3, "upward"],
      ["$$\nx\n$$", 3, "forward"],
      ["$$\nx\n$$", 3, "downward"],
      ["$$\nx\n$$\nEnd", 3, "backward"],
      ["Top\n$$\nx\n$$", 7, "forward"],
    ] as const) {
      const note = new Note(doc, at);
      const f = new Field(note);
      note.specs = [];
      leaveMaths(note as unknown as EditorView, writableTarget(note, f.text), dir);
      expect(note.doc).toBe(doc);
      expect(note.specs).toEqual([]);
      expect(visualMath(note.state)?.id).toBe(f.text.id);
    }
  });

  test("leaving towards a touching block opens its field at the near end", () => {
    const touching = "$$\na\n$$\n$$\nb\n$$";
    for (const dir of ["downward", "forward"] as const) {
      const note = new Note(touching, 3);
      leaveMaths(note as unknown as EditorView, writableTarget(note, new Field(note).text), dir);
      expect(note.doc).toBe(touching);
      expect(note.state.selection.main.head).toBe(8);
      expect(visualMath(note.state)?.start).toBe(8);
    }
    for (const dir of ["upward", "backward"] as const) {
      const note = new Note(touching, 11);
      leaveMaths(note as unknown as EditorView, writableTarget(note, new Field(note).text), dir);
      expect(note.doc).toBe(touching);
      expect(note.state.selection.main.head).toBe(7);
      expect(visualMath(note.state)?.start).toBe(0);
    }
  });
});

describe("Enter in a field", () => {
  const key = (shiftKey = false) => ({ key: "Enter", shiftKey, metaKey: false, ctrlKey: false, altKey: false });
  const at = (stop: number) => ({ anchor: stop, head: stop });

  test("in a block makes a line outside it: after, or before from its very start", () => {
    expect(enterSide(key(), true, "math", at(3))).toBe("after");
    expect(enterSide(key(), true, "text", at(1))).toBe("after");
    expect(enterSide(key(), true, "math", at(0))).toBe("before");
    expect(enterSide(key(), true, "math", { anchor: 0, head: 2 })).toBe("after");
  });

  test("Shift+Enter, inline maths and a `\\command` being typed are the model's", () => {
    expect(enterSide(key(true), true, "math", at(3))).toBeNull();
    expect(enterSide(key(), false, "math", at(3))).toBeNull();
    expect(enterSide(key(true), false, "math", at(3))).toBeNull();
    expect(enterSide(key(), true, "command", at(3))).toBeNull();
  });

  test("Shift+Enter (the model's Enter) adds a row, an environment's in one", () => {
    const note = new Note("Top\n$$\nx\n$$\nEnd", 7);
    new Field(note).run("enter");
    expect(note.doc).toBe("Top\n$$\nx \\\\\n$$\nEnd");
    const m = new Note("Top\n$$\n\\begin{cases}a & b\\end{cases}\n$$\nEnd", 22);
    const mf = new Field(m);
    mf.field = mf.field.caretAt(mf.field.source.indexOf("a") + 1, false);
    mf.run("enter");
    expect(m.doc).toBe("Top\n$$\n\\begin{cases}\na & b \\\\\n&\n\\end{cases}\n$$\nEnd");
  });

  const enter = (doc: string, at: number, before: boolean) => {
    const note = new Note(doc, at);
    const f = new Field(note);
    note.specs = [];
    newlineBeside(note as unknown as EditorView, writableTarget(note, f.text), before);
    return note;
  };

  test("mid-block and at its end, the line goes after the block, the caret on it", () => {
    for (const caret of [7, 8]) {
      const note = enter("Top\n$$\nx+y\n$$\nEnd", caret, false);
      expect(note.doc).toBe("Top\n$$\nx+y\n$$\n\nEnd");
      expect(note.state.selection.main.head).toBe(14);
      expect(visualMath(note.state)).toBeNull();
      expect(note.measures).toBe(1);
      expect(note.specs.some((s) => s.scrollIntoView)).toBe(false);
    }
  });

  test("from its very start, the line goes before it, so a note starting with a block gets text above", () => {
    const note = enter("$$\nx\n$$", 3, true);
    expect(note.doc).toBe("\n$$\nx\n$$");
    expect(note.state.selection.main.head).toBe(0);
    expect(visualMath(note.state)).toBeNull();
  });

  test("between stacked blocks, a line goes between them", () => {
    const note = enter("$$\na\n$$\n$$\nb\n$$", 3, false);
    expect(note.doc).toBe("$$\na\n$$\n\n$$\nb\n$$");
    expect(note.state.selection.main.head).toBe(8);
    expect(visualMath(note.state)).toBeNull();
  });

  test("is an undo step of its own", () => {
    const note = new Note("Top\n$$\nx\n$$\nEnd", 7);
    const f = new Field(note);
    f.type("y");
    newlineBeside(note as unknown as EditorView, writableTarget(note, f.text), false);
    note.undo();
    expect(note.doc).toBe("Top\n$$\nxy\n$$\nEnd");
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

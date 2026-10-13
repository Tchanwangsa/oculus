import { describe, expect, test } from "bun:test";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";

import { constructsIn } from "@/components/documents/VersionView/constructs";
import { diffPlan } from "@/components/documents/VersionView/diffPlan";
import { noteMarkdown } from "@/components/documents/editor/core/language";
import { diffHunks, diffText, type DiffChange, type DiffHunk } from "@/lib/notes/versionDiff";

function stateOf(doc: string) {
  const state = EditorState.create({ doc, extensions: [noteMarkdown()] });
  ensureSyntaxTree(state, doc.length, 5000);
  return state;
}

/** One hunk per change, which is all these cases need. */
function plan(a: string, b: string, changes: DiffChange[]) {
  const hunks: DiffHunk[] = changes.map((c) => ({ fromB: c.fromB, toB: c.toB, changes: [c] }));
  return diffPlan(stateOf(b), a, changes, hunks);
}

describe("text changes", () => {
  test("an insertion is a span and an ins line", () => {
    const p = plan("Hello world", "Hello brave world", [{ fromA: 6, toA: 6, fromB: 6, toB: 12 }]);
    expect(p.inserts).toEqual([{ from: 6, to: 12, hunk: 0 }]);
    expect(p.deletions).toEqual([]);
    expect(p.lines).toEqual([{ at: 0, kind: "ins", hunks: [0] }]);
  });

  test("a deletion within a line is inline, before what replaced it", () => {
    const p = plan("Hello brave world", "Hello bold world", [{ fromA: 6, toA: 11, fromB: 6, toB: 10 }]);
    expect(p.deletions).toEqual([{ at: 6, text: "brave", block: false, side: -1, hunk: 0 }]);
    expect(p.lines).toEqual([{ at: 0, kind: "mix", hunks: [0] }]);
  });

  test("a deleted line draws as a block before the line that followed it", () => {
    const p = plan("A\nB\nC", "A\nC", [{ fromA: 2, toA: 4, fromB: 2, toB: 2 }]);
    expect(p.deletions).toEqual([{ at: 2, text: "B", block: true, side: -1, hunk: 0 }]);
    expect(p.lines).toEqual([]);
  });

  test("deleted lines hanging off a line's end draw after it", () => {
    const p = plan("A\nB\nC", "A", [{ fromA: 1, toA: 5, fromB: 1, toB: 1 }]);
    expect(p.deletions).toEqual([{ at: 1, text: "B\nC", block: true, side: 1, hunk: 0 }]);
  });

  test("an inserted multi-line span marks every line it covers", () => {
    const p = plan("A", "A\nB\nC\n", [{ fromA: 1, toA: 1, fromB: 1, toB: 6 }]);
    expect(p.lines.map((l) => l.at)).toEqual([0, 2, 4]);
  });
});

describe("constructs drawn as widgets", () => {
  test("a change inside inline maths tints the whole maths", () => {
    const p = plan("x $a+b$ y", "x $a+c$ y", [{ fromA: 6, toA: 7, fromB: 6, toB: 7 }]);
    expect(p.constructs).toEqual([{ from: 2, to: 7, block: false, kind: "mod", hunk: 0 }]);
    expect(p.inserts).toEqual([]);
    expect(p.deletions).toEqual([]);
    expect(p.lines).toEqual([{ at: 0, kind: "mix", hunks: [0] }]);
  });

  test("a change in a table's cell tints the table, whose lines get no bar", () => {
    const a = "Intro\n\n| a | b |\n| - | - |\n| 1 | 2 |\n";
    const b = "Intro\n\n| a | b |\n| - | - |\n| 1 | 3 |\n";
    const at = b.indexOf("3");
    const p = plan(a, b, [{ fromA: at, toA: at + 1, fromB: at, toB: at + 1 }]);
    expect(p.constructs).toEqual([{ from: 7, to: b.length - 1, block: true, kind: "mod", hunk: 0 }]);
    expect(p.lines).toEqual([]);
  });

  test("a new display-maths block is an inserted construct", () => {
    const a = "p\n\n";
    const b = "p\n\n$$\nx^2\n$$\n";
    const p = plan(a, b, [{ fromA: 3, toA: 3, fromB: 3, toB: b.length }]);
    expect(p.constructs).toEqual([{ from: 3, to: b.length - 1, block: true, kind: "ins", hunk: 0 }]);
    // Only the closing newline lies outside the block, and a mark over it
    // would draw nothing.
    expect(p.inserts).toEqual([]);
  });

  test("a deletion inside a picture's link is the picture's", () => {
    const b = "See ![a](x.png) here";
    const p = plan("See ![a](xy.png) here", b, [{ fromA: 10, toA: 11, fromB: 10, toB: 10 }]);
    expect(p.deletions).toEqual([]);
    expect(p.constructs).toEqual([{ from: 4, to: 15, block: false, kind: "mod", hunk: 0 }]);
  });

  test("an edge of a construct is not inside it", () => {
    expect(constructsIn(stateOf("x $a$ y"), 2, 2)).toEqual([]);
    expect(constructsIn(stateOf("x $a$ y"), 5, 5)).toEqual([]);
    expect(constructsIn(stateOf("x $a$ y"), 3, 3)).toEqual([{ from: 2, to: 5, block: false }]);
  });

  test("a task's checkbox and a citation chip are constructs", () => {
    const task = stateOf("- [x] done");
    expect(constructsIn(task, 3, 4)).toEqual([{ from: 2, to: 5, block: false }]);
    const cite = stateOf("see `courses/A/files/w.pdf` now");
    expect(constructsIn(cite, 10, 11)).toEqual([{ from: 4, to: 27, block: false }]);
  });
});

describe("marks beside inline maths", () => {
  const before = "Prepare the superposition $|\\psi\\rangle = \\frac{1}{N}\\sum_x |x\\rangle$ next.";
  const after = "Prepare the superposition $|\\psi_0\\rangle = \\frac{1}{\\sqrt N}\\sum_x |x\\rangle$ next.";
  const maths = { from: after.indexOf("$"), to: after.lastIndexOf("$") + 1, block: false, kind: "mod" as const, hunk: 0 };

  test("a change running from the space before the maths into it draws only the maths' tint", () => {
    const space = after.indexOf(" $");
    const p = plan(before, after, [
      { fromA: space, toA: before.indexOf("\\rangle"), fromB: space, toB: after.indexOf("\\rangle") },
      { fromA: before.indexOf("{N}") + 1, toA: before.indexOf("{N}") + 1, fromB: after.indexOf("\\sqrt"), toB: after.indexOf("N}") },
    ]);
    expect(p.constructs).toEqual([maths]);
    expect(p.inserts).toEqual([]);
    expect(p.deletions).toEqual([]);
    expect(p.lines).toEqual([{ at: 0, kind: "mix", hunks: [0] }]);
  });

  test("a space changed just before the maths folds into it", () => {
    const a = "Prepare the superposition  $x$ next.";
    const b = "Prepare the superposition $x$ next.";
    const at = b.indexOf("$");
    const p = plan(a, b, [{ fromA: at, toA: at + 1, fromB: at, toB: at }]);
    expect(p.constructs).toEqual([{ from: at, to: at + 3, block: false, kind: "mod", hunk: 0 }]);
    expect(p.inserts).toEqual([]);
    expect(p.deletions).toEqual([]);
  });

  test("the real word diff of that edit draws one tint and nothing else", () => {
    const changes = diffText(before, after);
    const p = diffPlan(stateOf(after), before, changes, diffHunks(changes, after));
    expect(p.constructs).toEqual([maths]);
    expect(p.inserts).toEqual([]);
    expect(p.deletions).toEqual([]);
  });

  test("a change over text and the start of maths keeps the text's marks, trimmed at the maths", () => {
    const a = "the old $x+y$ end";
    const b = "the new $z+y$ end";
    const p = plan(a, b, [{ fromA: 4, toA: 10, fromB: 4, toB: 10 }]);
    expect(p.constructs).toEqual([{ from: 8, to: 13, block: false, kind: "mod", hunk: 0 }]);
    expect(p.inserts).toEqual([{ from: 4, to: 7, hunk: 0 }]);
    expect(p.deletions).toEqual([{ at: 4, text: "old ", block: false, side: -1, hunk: 0 }]);
  });

  test("text deleted after the maths goes just past it", () => {
    const a = "see $x$ gone end";
    const b = "see $y$ end";
    const p = plan(a, b, [{ fromA: 5, toA: 12, fromB: 5, toB: 7 }]);
    expect(p.deletions).toEqual([{ at: 7, text: " gone", block: false, side: 1, hunk: 0 }]);
    expect(p.inserts).toEqual([]);
  });
});

describe("empty marks", () => {
  test("whitespace or hidden markup alone gets no mark, only the line bar", () => {
    const p = plan("a b", "a  b", [{ fromA: 1, toA: 1, fromB: 1, toB: 2 }]);
    expect(p.inserts).toEqual([]);
    expect(p.lines).toEqual([{ at: 0, kind: "ins", hunks: [0] }]);
    const bold = plan("make it bold", "make it **bold**", [
      { fromA: 8, toA: 8, fromB: 8, toB: 10 },
      { fromA: 12, toA: 12, fromB: 14, toB: 16 },
    ]);
    expect(bold.inserts).toEqual([]);
    expect(plan("a  b", "a b", [{ fromA: 1, toA: 2, fromB: 1, toB: 1 }]).deletions).toEqual([]);
  });

  test("abutting inserts in one hunk merge", () => {
    const changes = [
      { fromA: 2, toA: 2, fromB: 2, toB: 5 },
      { fromA: 2, toA: 2, fromB: 5, toB: 8 },
    ];
    const hunks = [{ fromB: 2, toB: 8, changes }];
    expect(diffPlan(stateOf("a xyzuvw b"), "a b", changes, hunks).inserts).toEqual([{ from: 2, to: 8, hunk: 0 }]);
  });
});

import { describe, expect, test } from "bun:test";

import {
  backspaceEdit,
  closeEdit,
  groupEnv,
  joinLatex,
  pad,
  semicolonEdit,
  spaceEdit,
  toLatex,
  trim,
  type CellCaret,
  type Grid,
  type GridEdit,
} from "../src/components/documents/editor/mathMatrix";

/** The caret at the end of a cell's content, after a term. */
function atEnd(grid: Grid, row: number, col: number, extra: Partial<CellCaret> = {}): CellCaret {
  const before = grid.rows[row][col];
  return { row, col, before, after: "", follows: before ? "term" : "start", lone: false, ...extra };
}

/** Apply an edit to a grid, as the field would: the new grid and the caret. */
function step(grid: Grid, edit: GridEdit | null): { grid: Grid; cell: [number, number] } {
  if (!edit) throw new Error("no edit");
  if (edit.kind === "close") return { grid: edit.grid, cell: [-1, -1] };
  return { grid: edit.kind === "edit" ? edit.grid : grid, cell: edit.cell };
}

/** `text` typed at the end of a cell. */
function type(grid: Grid, [r, c]: [number, number], text: string): Grid {
  const rows = grid.rows.map((row) => [...row]);
  rows[r][c] += text;
  return { env: grid.env, rows };
}

describe("bracket groups", () => {
  test("matching or ghost right delimiters draw a matrix; mismatched ones don't", () => {
    expect(groupEnv("(", "?")).toBe("pmatrix");
    expect(groupEnv("(", ")")).toBe("pmatrix");
    expect(groupEnv("\\lbrack", "\\rbrack")).toBe("bmatrix");
    expect(groupEnv("[", "]")).toBe("bmatrix");
    expect(groupEnv("\\{", "\\}")).toBe("Bmatrix");
    expect(groupEnv("\\lbrace", "?")).toBe("Bmatrix");
    expect(groupEnv("|", "|")).toBe("vmatrix");
    expect(groupEnv("\\lvert", "\\rvert")).toBe("vmatrix");
    expect(groupEnv("\\Vert", "\\Vert")).toBe("Vmatrix");
    expect(groupEnv("[", ")")).toBeNull();
    expect(groupEnv("\\langle", "\\rangle")).toBeNull();
  });

  test("a one-cell grid is written as its bracket group, a ghost unless closed", () => {
    expect(toLatex({ env: "pmatrix", rows: [["a+b"]] })).toBe("\\left( a+b\\right?");
    expect(toLatex({ env: "bmatrix", rows: [["x"]] }, true)).toBe("\\left\\lbrack x\\right\\rbrack");
    // No brackets to fall back to: stays a matrix.
    expect(toLatex({ env: "matrix", rows: [["x"]] })).toBe("\\begin{matrix}x\\end{matrix}");
  });

  test("empty cells are placeholders", () => {
    expect(toLatex({ env: "bmatrix", rows: [["a", ""], ["", "d"]] })).toBe(
      "\\begin{bmatrix}a & \\placeholder{}\\\\ \\placeholder{} & d\\end{bmatrix}",
    );
  });

  test("a control word before a letter keeps a space", () => {
    expect(joinLatex("\\alpha", "b")).toBe("\\alpha b");
    expect(joinLatex("\\alpha", "+")).toBe("\\alpha+");
    expect(joinLatex("a", "\\le")).toBe("a\\le");
  });
});

describe("Space", () => {
  test("after a term in a bracket group starts a second cell", () => {
    const grid = { env: "pmatrix", rows: [["a"]] };
    expect(spaceEdit(grid, atEnd(grid, 0, 0))).toEqual({
      kind: "edit",
      grid: { env: "pmatrix", rows: [["a", ""]] },
      cell: [0, 1],
      at: "start",
    });
  });

  test("leaves an empty cell, an operator's end or a cell's start to the toolbox", () => {
    const empty = { env: "pmatrix", rows: [[""]] };
    expect(spaceEdit(empty, atEnd(empty, 0, 0))).toBeNull();
    const op = { env: "bmatrix", rows: [["a+"]] };
    expect(spaceEdit(op, atEnd(op, 0, 0, { follows: "operator" }))).toBeNull();
    const start = { env: "bmatrix", rows: [["a"]] };
    expect(spaceEdit(start, { row: 0, col: 0, before: "", after: "a", follows: "start", lone: false })).toBeNull();
  });

  test("`(a + b)` with habitual spaces stays one cell, back to plain brackets", () => {
    let grid: Grid = { env: "pmatrix", rows: [["a"]] };
    let r = step(grid, spaceEdit(grid, atEnd(grid, 0, 0)));
    grid = type(r.grid, r.cell, "+");
    r = step(grid, spaceEdit(grid, atEnd(grid, 0, 1, { follows: "operator", lone: true })));
    expect(r.grid).toEqual({ env: "pmatrix", rows: [["a+"]] });
    expect(r.cell).toEqual([0, 0]);
    expect(toLatex(type(r.grid, r.cell, "b"))).toBe("\\left( a+b\\right?");
  });

  test("`[1 -1]` keeps two cells", () => {
    let grid: Grid = { env: "bmatrix", rows: [["1"]] };
    let r = step(grid, spaceEdit(grid, atEnd(grid, 0, 0)));
    grid = type(r.grid, r.cell, "-1");
    expect(grid.rows).toEqual([["1", "-1"]]);
    r = step(grid, closeEdit(grid));
    expect(r.grid.rows).toEqual([["1", "-1"]]);
  });

  test("an operator merged in a row of a taller grid shifts the row's later cells left", () => {
    const grid = { env: "bmatrix", rows: [["a", "b", "c"], ["d", "=", "e"]] };
    const r = spaceEdit(grid, atEnd(grid, 1, 1, { follows: "operator", lone: true }));
    expect(r).toEqual({ kind: "edit", grid: { env: "bmatrix", rows: [["a", "b", "c"], ["d=", "e", ""]] }, cell: [1, 0], at: "end" });
  });

  test("an operator alone in a row's first cell is not merged", () => {
    const grid = { env: "bmatrix", rows: [["+"]] };
    expect(spaceEdit(grid, atEnd(grid, 0, 0, { follows: "operator", lone: true }))).toBeNull();
  });

  test("mid-cell splits off what follows the caret into a new column", () => {
    const grid = { env: "bmatrix", rows: [["ab", "c"], ["d", "e"]] };
    expect(spaceEdit(grid, { row: 0, col: 0, before: "a", after: "b", follows: "term", lone: false })).toEqual({
      kind: "edit",
      grid: { env: "bmatrix", rows: [["a", "b", "c"], ["d", "", "e"]] },
      cell: [0, 1],
      at: "start",
    });
  });

  test("at a cell's end with an empty next cell, only moves into it", () => {
    const grid = { env: "bmatrix", rows: [["a", "b"], ["c", ""]] };
    expect(spaceEdit(grid, atEnd(grid, 1, 0))).toEqual({ kind: "move", cell: [1, 1], at: "start" });
  });

  test("stops at MathLive's ten columns", () => {
    const grid = { env: "bmatrix", rows: [["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]] };
    expect(spaceEdit(grid, atEnd(grid, 0, 9))).toBeNull();
  });

  test("`[a b; c d e f g]` pads the first row as the second grows", () => {
    let grid: Grid = { env: "bmatrix", rows: [["a"]] };
    let r = step(grid, spaceEdit(grid, atEnd(grid, 0, 0)));
    grid = type(r.grid, r.cell, "b");
    r = step(grid, semicolonEdit(grid, { row: 0 }));
    expect(r.grid.rows).toEqual([["a", "b"], ["", ""]]);
    let cell = r.cell;
    grid = r.grid;
    for (const text of ["c", "d", "e", "f"]) {
      grid = type(grid, cell, text);
      r = step(grid, spaceEdit(grid, atEnd(grid, cell[0], cell[1])));
      grid = r.grid;
      cell = r.cell;
    }
    grid = type(grid, cell, "g");
    expect(grid.rows).toEqual([
      ["a", "b", "", "", ""],
      ["c", "d", "e", "f", "g"],
    ]);
    expect(closeEdit(grid)).toEqual({ kind: "close", grid });
  });

  test("a ragged grid read from LaTeX is padded before a split", () => {
    const grid = { env: "bmatrix", rows: [["a", "b"], ["c", "d", "e", "f", "g"]] };
    expect(spaceEdit(grid, atEnd(grid, 0, 1))).toEqual({ kind: "move", cell: [0, 2], at: "start" });
    const r = spaceEdit(grid, atEnd(grid, 1, 4));
    expect(r?.kind === "edit" && r.grid.rows).toEqual([
      ["a", "b", "", "", "", ""],
      ["c", "d", "e", "f", "g", ""],
    ]);
    expect(pad(grid).rows[0]).toEqual(["a", "b", "", "", ""]);
  });
});

describe("`;`", () => {
  test("in a bracket group adds a row", () => {
    const grid = { env: "pmatrix", rows: [["x"]] };
    expect(semicolonEdit(grid, { row: 0 })).toEqual({
      kind: "edit",
      grid: { env: "pmatrix", rows: [["x"], [""]] },
      cell: [1, 0],
      at: "start",
    });
  });

  test("in a middle row inserts an empty row after it, or moves into an empty one", () => {
    const grid = { env: "bmatrix", rows: [["a", "b"], ["c", "d"], ["e", "f"]] };
    expect(semicolonEdit(grid, { row: 0 })).toEqual({
      kind: "edit",
      grid: { env: "bmatrix", rows: [["a", "b"], ["", ""], ["c", "d"], ["e", "f"]] },
      cell: [1, 0],
      at: "start",
    });
    const gap = { env: "bmatrix", rows: [["a", "b"], ["", ""], ["e", "f"]] };
    expect(semicolonEdit(gap, { row: 0 })).toEqual({ kind: "move", cell: [1, 0], at: "start" });
  });

  test("new rows take the widest row's width", () => {
    const grid = { env: "bmatrix", rows: [["a"], ["b", "c", "d"]] };
    const r = semicolonEdit(grid, { row: 1 });
    expect(r.kind === "edit" && r.grid.rows).toEqual([
      ["a", "", ""],
      ["b", "c", "d"],
      ["", "", ""],
    ]);
  });
});

describe("Backspace", () => {
  const empty = (row: number, col: number): CellCaret => ({ row, col, before: "", after: "", follows: "start", lone: false });

  test("in an empty column removes it; back to the bracket group at one cell", () => {
    const grid = { env: "pmatrix", rows: [["a", ""]] };
    expect(backspaceEdit(grid, empty(0, 1))).toEqual({ kind: "edit", grid: { env: "pmatrix", rows: [["a"]] }, cell: [0, 0], at: "end" });
    expect(toLatex({ env: "pmatrix", rows: [["a"]] })).toBe("\\left( a\\right?");
  });

  test("removes the column when it is all empty, else the row", () => {
    const col = { env: "bmatrix", rows: [["a", "", "b"], ["c", "", "d"]] };
    expect(backspaceEdit(col, empty(1, 1))).toEqual({
      kind: "edit",
      grid: { env: "bmatrix", rows: [["a", "b"], ["c", "d"]] },
      cell: [1, 0],
      at: "end",
    });
    const row = { env: "bmatrix", rows: [["a", "b"], ["", ""]] };
    expect(backspaceEdit(row, empty(1, 1))).toEqual({ kind: "edit", grid: { env: "bmatrix", rows: [["a", "b"]] }, cell: [0, 1], at: "end" });
  });

  test("a first column removed puts the caret at the previous row's end", () => {
    const grid = { env: "bmatrix", rows: [["", "a"], ["", "b"]] };
    expect(backspaceEdit(grid, empty(1, 0))).toEqual({ kind: "edit", grid: { env: "bmatrix", rows: [["a"], ["b"]] }, cell: [0, 0], at: "end" });
    expect(backspaceEdit(grid, empty(0, 0))).toEqual({ kind: "edit", grid: { env: "bmatrix", rows: [["a"], ["b"]] }, cell: [0, 0], at: "start" });
  });

  test("an empty one-column row collapses a column vector to its group", () => {
    const grid = { env: "pmatrix", rows: [["x"], [""]] };
    expect(backspaceEdit(grid, empty(1, 0))).toEqual({ kind: "edit", grid: { env: "pmatrix", rows: [["x"]] }, cell: [0, 0], at: "end" });
  });

  test("an empty cell beside full ones steps back a cell, row-major", () => {
    const grid = { env: "bmatrix", rows: [["a", "b"], ["", "d"]] };
    expect(backspaceEdit(grid, empty(1, 0))).toEqual({ kind: "move", cell: [0, 1], at: "end" });
    const mid = { env: "bmatrix", rows: [["a", ""], ["c", "d"]] };
    expect(backspaceEdit(mid, empty(0, 1))).toEqual({ kind: "move", cell: [0, 0], at: "end" });
    const first = { env: "bmatrix", rows: [["", "b"], ["c", "d"]] };
    expect(backspaceEdit(first, empty(0, 0))).toBeNull();
  });

  test("leaves a cell with content, and a one-cell grid, to MathLive", () => {
    const grid = { env: "bmatrix", rows: [["a", "b"]] };
    expect(backspaceEdit(grid, { row: 0, col: 1, before: "", after: "b", follows: "start", lone: false })).toBeNull();
    expect(backspaceEdit({ env: "pmatrix", rows: [[""]] }, empty(0, 0))).toBeNull();
  });
});

describe("closing bracket", () => {
  test("drops trailing empty rows and columns", () => {
    const grid = { env: "bmatrix", rows: [["a", "b", ""], ["c", "", ""], ["", "", ""]] };
    expect(closeEdit(grid)).toEqual({ kind: "close", grid: { env: "bmatrix", rows: [["a", "b"], ["c", ""]] } });
  });

  test("a single cell left closes as a bracket group", () => {
    const grid = trim({ env: "pmatrix", rows: [["a", ""], ["", ""]] });
    expect(grid.rows).toEqual([["a"]]);
    expect(toLatex(grid, true)).toBe("\\left( a\\right)");
  });
});

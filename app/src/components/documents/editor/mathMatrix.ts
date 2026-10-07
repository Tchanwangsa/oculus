/**
 * Matrices typed as in MATLAB (`[a b; c d]`) in the visual maths field: Space
 * between terms starts a cell, `;` a row, Backspace in an empty cell takes
 * its column or row back, and the closing bracket trims empty ends. A grid is
 * a matrix environment's cells, or a bracket group as one cell, which the
 * first new cell turns into the matrix its brackets draw. Pure: the field's
 * side, reading MathLive's atoms and writing the result, is `mathMatrixField.ts`.
 */

/** A matrix's environment and each cell's LaTeX, "" for an empty cell. */
export interface Grid {
  env: string;
  rows: string[][];
}

/** The environments typed as grids: matrices, not `array`, `cases` or
 *  `aligned`, whose cells hold more than terms. */
export const MATRIX_ENVS = new Set(["matrix", "pmatrix", "bmatrix", "Bmatrix", "vmatrix", "Vmatrix", "smallmatrix"]);

/** MathLive wraps a row past its tenth column onto a new row. */
export const MAX_COLS = 10;

const LEFT_ENV: Record<string, string> = {
  "(": "pmatrix",
  "\\lparen": "pmatrix",
  "[": "bmatrix",
  "\\lbrack": "bmatrix",
  "\\{": "Bmatrix",
  "\\lbrace": "Bmatrix",
  "|": "vmatrix",
  "\\vert": "vmatrix",
  "\\lvert": "vmatrix",
  "\\|": "Vmatrix",
  "\\Vert": "Vmatrix",
  "\\lVert": "Vmatrix",
};

const RIGHT_ENV: Record<string, string> = {
  ")": "pmatrix",
  "\\rparen": "pmatrix",
  "]": "bmatrix",
  "\\rbrack": "bmatrix",
  "\\}": "Bmatrix",
  "\\rbrace": "Bmatrix",
  "|": "vmatrix",
  "\\vert": "vmatrix",
  "\\rvert": "vmatrix",
  "\\|": "Vmatrix",
  "\\Vert": "Vmatrix",
  "\\rVert": "Vmatrix",
};

/** The delimiters a matrix has as a bracket group: what typing them gives. */
const GROUP_DELIMS: Record<string, [string, string]> = {
  pmatrix: ["(", ")"],
  bmatrix: ["\\lbrack", "\\rbrack"],
  Bmatrix: ["\\lbrace", "\\rbrace"],
  vmatrix: ["|", "|"],
  Vmatrix: ["\\Vert", "\\Vert"],
};

/** The key that closes a matrix as its right bracket. */
export const CLOSE_KEYS: Record<string, string> = { pmatrix: ")", bmatrix: "]", Bmatrix: "}", vmatrix: "|" };

/** The matrix a bracket group's delimiters draw, or null when they don't
 *  match (`[0,1)`). A right delimiter `?` is MathLive's ghost of one not yet
 *  typed. */
export function groupEnv(left: string, right: string): string | null {
  const env = LEFT_ENV[left];
  return env && (right === "?" || RIGHT_ENV[right] === env) ? env : null;
}

/** Whether a grid of this shape is written as a bracket group: one cell, in
 *  an environment that has one. */
export function isGroup(grid: Grid): boolean {
  return grid.rows.length === 1 && grid.rows[0].length === 1 && grid.env in GROUP_DELIMS;
}

/** `a` then `b` as one run of LaTeX: a control word before a letter takes a
 *  space, or `\alpha` and `b` would read as `\alphab`. */
export function joinLatex(a: string, b: string): string {
  return /\\[a-zA-Z]+$/.test(a) && /^[a-zA-Z]/.test(b) ? `${a} ${b}` : a + b;
}

/** The grid as LaTeX: an environment whose empty cells are placeholders, so
 *  they show as slots and Tab reaches them; a bracket group (`isGroup`) as
 *  `\left…\right`, its right delimiter the ghost `?` unless `closed`. */
export function toLatex(grid: Grid, closed = false): string {
  const delims = GROUP_DELIMS[grid.env];
  if (isGroup(grid) && delims) {
    return `\\left${delims[0]} ${grid.rows[0][0]}\\right${closed ? delims[1] : "?"}`;
  }
  const rows = grid.rows.map((row) => row.map((cell) => cell || "\\placeholder{}").join(" & "));
  return `\\begin{${grid.env}}${rows.join("\\\\ ")}\\end{${grid.env}}`;
}

/** Every row as wide as the widest, with empty cells. */
export function pad(grid: Grid): Grid {
  const width = Math.max(1, ...grid.rows.map((row) => row.length));
  return { env: grid.env, rows: grid.rows.map((row) => [...row, ...Array<string>(width - row.length).fill("")]) };
}

const width = (grid: Grid) => grid.rows[0]?.length ?? 0;
const emptyRow = (grid: Grid, r: number) => grid.rows[r].every((cell) => !cell);
const emptyCol = (grid: Grid, c: number) => grid.rows.every((row) => !row[c]);

/** Without trailing rows and columns that are all empty, keeping one. */
export function trim(grid: Grid): Grid {
  const g = pad(grid);
  while (g.rows.length > 1 && emptyRow(g, g.rows.length - 1)) g.rows.pop();
  while (width(g) > 1 && emptyCol(g, width(g) - 1)) for (const row of g.rows) row.pop();
  return g;
}

/** The caret in a grid's cell. */
export interface CellCaret {
  row: number;
  col: number;
  /** The cell's LaTeX before and after the caret. */
  before: string;
  after: string;
  /** What the caret comes after in its cell: nothing, something that wants
   *  what follows (an operator, relation, punctuation, opening), or a term. */
  follows: "start" | "operator" | "term";
  /** The cell is one binary operator or relation and nothing else. */
  lone: boolean;
}

/** A cell and which end of it the caret goes to. */
export interface CellTarget {
  cell: [number, number];
  at: "start" | "end";
}

/** What a key does to a grid: rewrite it (`grid`, written as a bracket group
 *  when `isGroup`) with the caret in a cell, only move the caret, or close
 *  it with the caret after it. */
export type GridEdit =
  | ({ kind: "edit"; grid: Grid } & CellTarget)
  | ({ kind: "move" } & CellTarget)
  | { kind: "close"; grid: Grid };

const copy = (grid: Grid): Grid => pad({ env: grid.env, rows: grid.rows.map((row) => [...row]) });

/**
 * Space in a grid's cell. An operator alone in a cell after the row's first
 * rejoins the cell before it (`[a + b]` is one cell, `[1 -1]` two); after a
 * term it ends the cell — into the next one when it is empty and the caret
 * at the cell's end, else into a new column holding what was after the
 * caret. Null leaves Space to the toolbox: an empty cell, after an operator.
 */
export function spaceEdit(grid: Grid, caret: CellCaret): GridEdit | null {
  const { row: r, col: c, before, after } = caret;
  if (!before && !after) return null;
  const g = copy(grid);
  if (caret.lone && c > 0) {
    const cells = g.rows[r];
    cells[c - 1] = joinLatex(cells[c - 1], joinLatex(before, after));
    cells[c] = "";
    // Its column goes when nothing else is in it, else the row's later
    // cells shift left into it.
    if (emptyCol(g, c)) for (const row of g.rows) row.splice(c, 1);
    else cells.push(...cells.splice(c, 1));
    return { kind: "edit", grid: g, cell: [r, c - 1], at: "end" };
  }
  if (caret.follows !== "term") return null;
  if (!after && c + 1 < width(g) && !g.rows[r][c + 1]) return { kind: "move", cell: [r, c + 1], at: "start" };
  if (width(g) >= MAX_COLS) return null;
  for (const row of g.rows) row.splice(c + 1, 0, "");
  g.rows[r][c] = before;
  g.rows[r][c + 1] = after;
  return { kind: "edit", grid: g, cell: [r, c + 1], at: "start" };
}

/** `;` in a grid's cell: the caret goes to the start of the row after its
 *  own — a new empty row unless that row is already empty. */
export function semicolonEdit(grid: Grid, caret: Pick<CellCaret, "row">): GridEdit {
  const g = copy(grid);
  const r = caret.row;
  if (r + 1 < g.rows.length && emptyRow(g, r + 1)) return { kind: "move", cell: [r + 1, 0], at: "start" };
  g.rows.splice(r + 1, 0, Array<string>(width(g)).fill(""));
  return { kind: "edit", grid: g, cell: [r + 1, 0], at: "start" };
}

/**
 * Backspace at the start of an empty cell, in a grid of more than one: its
 * column goes when all empty, else its row when all empty, else the caret
 * steps back to the end of the cell before. Null in the first cell, a
 * cell with anything in it, or a one-cell grid: MathLive's own Backspace.
 */
export function backspaceEdit(grid: Grid, caret: CellCaret): GridEdit | null {
  const { row: r, col: c } = caret;
  if (caret.before || caret.after) return null;
  const g = copy(grid);
  const rows = g.rows.length;
  const cols = width(g);
  if (rows * cols <= 1) return null;
  // After a removal: the cell before in the row, else the previous row's end.
  const back = (rr: number, cc: number): CellTarget =>
    cc >= 0 ? { cell: [rr, cc], at: "end" } : rr > 0 ? { cell: [rr - 1, width(g) - 1], at: "end" } : { cell: [0, 0], at: "start" };
  if (cols > 1 && emptyCol(g, c)) {
    for (const row of g.rows) row.splice(c, 1);
    return { kind: "edit", grid: g, ...back(r, c - 1) };
  }
  if (rows > 1 && emptyRow(g, r)) {
    g.rows.splice(r, 1);
    return { kind: "edit", grid: g, ...back(r, -1) };
  }
  if (c > 0) return { kind: "move", cell: [r, c - 1], at: "end" };
  if (r > 0) return { kind: "move", cell: [r - 1, cols - 1], at: "end" };
  return null;
}

/** The closing bracket: the matrix without trailing empty rows and columns,
 *  the caret after it (a single cell left is a closed bracket group). */
export function closeEdit(grid: Grid): GridEdit {
  return { kind: "close", grid: trim(grid) };
}

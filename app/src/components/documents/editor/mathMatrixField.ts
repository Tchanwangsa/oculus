import type { MathfieldElement } from "mathlive";

import type { MlAtom, MlModel } from "./mathField";
import {
  CLOSE_KEYS,
  MATRIX_ENVS,
  backspaceEdit,
  closeEdit,
  groupEnv,
  semicolonEdit,
  spaceEdit,
  toLatex,
  type CellCaret,
  type CellTarget,
  type Grid,
  type GridEdit,
} from "./mathMatrix";

/**
 * The visual field's side of MATLAB-style matrix typing (`mathMatrix.ts`):
 * the grid at the caret read from MathLive's atoms, and an edit written back
 * by replacing the whole structure with new LaTeX. An offset is the caret
 * after its atom; a structure's descendants come before it, so a cell runs
 * from its `first` atom's offset to its last atom's.
 */

/** The grid around the caret and the structure that holds it. */
export interface GridAt {
  /** A matrix's array atom, or a bracket group's `leftright` atom. */
  atom: MlAtom;
  /** A bracket group, not yet a matrix. */
  group: boolean;
  grid: Grid;
  caret: CellCaret;
}

/** A key's edit to the grid at the caret. */
export interface GridStep {
  at: GridAt;
  edit: GridEdit;
}

/** Atoms after which Space isn't a separator: they want what follows. Big
 *  operators and `\sin`-like ones too (MathLive's `extensible-symbol`,
 *  `operator`, `mop`), so `\sin x` stays one cell. */
const WANTS_OPERAND = new Set(["mbin", "mrel", "mpunct", "mopen", "mop", "operator", "extensible-symbol"]);

const CLOSERS = new Set(Object.values(CLOSE_KEYS));

/** The caret's offset: a collapsed selection's, or the start of a selected
 *  placeholder (where Tab, and a new cell, leave it). */
function caretOffset(mf: MathfieldElement, model: MlModel): number | null {
  const { ranges } = mf.selection;
  if (ranges.length !== 1) return null;
  const [a, b] = ranges[0][0] <= ranges[0][1] ? ranges[0] : [ranges[0][1], ranges[0][0]];
  if (a === b) return a;
  return b - a === 1 && model.at(b)?.type === "placeholder" ? a : null;
}

/** The LaTeX between two offsets, "" when only placeholders. */
function latexBetween(mf: MathfieldElement, from: number, to: number): string {
  if (from >= to || !mf.getValue(from, to, "latex-without-placeholders")) return "";
  return mf.getValue(from, to, "latex");
}

function cellLatex(mf: MathfieldElement, model: MlModel, cell: MlAtom[] | undefined): string {
  return cell?.length ? latexBetween(mf, model.offsetOf(cell[0]), model.offsetOf(cell[cell.length - 1])) : "";
}

/** The grid the caret is directly in: a cell of a matrix, or the body of a
 *  bracket group whose delimiters draw one. Math mode only. */
export function gridAt(mf: MathfieldElement, model: MlModel): GridAt | null {
  if (mf.mode !== "math") return null;
  const pos = caretOffset(mf, model);
  const here = pos == null ? undefined : model.at(pos);
  const parent = here?.parent;
  if (pos == null || !here || !parent) return null;
  let cell: MlAtom[] | undefined;
  let grid: Grid;
  let row = 0;
  let col = 0;
  if (parent.type === "leftright" && here.parentBranch === "body") {
    const env = groupEnv(parent.leftDelim ?? "", parent.rightDelim ?? "");
    if (!env) return null;
    cell = parent.body;
    grid = { env, rows: [[cellLatex(mf, model, cell)]] };
  } else if (parent.type === "array" && MATRIX_ENVS.has(parent.environmentName ?? "") && Array.isArray(here.parentBranch)) {
    [row, col] = here.parentBranch as [number, number];
    cell = parent.rows?.[row]?.[col];
    grid = { env: parent.environmentName!, rows: (parent.rows ?? []).map((r) => r.map((c) => cellLatex(mf, model, c))) };
  } else return null;
  if (!cell?.length) return null;
  const before = latexBetween(mf, model.offsetOf(cell[0]), pos);
  const after = latexBetween(mf, pos, model.offsetOf(cell[cell.length - 1]));
  const atoms = cell.filter((a) => a.type !== "first" && a.type !== "placeholder");
  return {
    atom: parent,
    group: parent.type === "leftright",
    grid,
    caret: {
      row,
      col,
      before,
      after,
      follows: !before ? "start" : WANTS_OPERAND.has(here.type) ? "operator" : "term",
      lone: atoms.length === 1 && (atoms[0].type === "mbin" || atoms[0].type === "mrel"),
    },
  };
}

/** What a plain key does in the grid at the caret, or null when it isn't a
 *  grid key there. The closing key closes a matrix, never a bracket group,
 *  which MathLive closes itself. */
export function gridKey(mf: MathfieldElement, model: MlModel, key: string): GridStep | null {
  if (key !== " " && key !== ";" && key !== "Backspace" && !CLOSERS.has(key)) return null;
  const at = gridAt(mf, model);
  if (!at) return null;
  let edit: GridEdit | null = null;
  if (key === " ") edit = spaceEdit(at.grid, at.caret);
  else if (key === ";") edit = semicolonEdit(at.grid, at.caret);
  else if (key === "Backspace") edit = backspaceEdit(at.grid, at.caret);
  else if (!at.group && CLOSE_KEYS[at.grid.env] === key) edit = closeEdit(at.grid);
  return edit && { at, edit };
}

/** The structure's scripts (`^T` on a matrix), carried onto its new LaTeX. */
function scripts(mf: MathfieldElement, model: MlModel, atom: MlAtom): string {
  let out = "";
  for (const [branch, mark] of [["subscript", "_"], ["superscript", "^"]] as const) {
    const atoms = atom.branch(branch);
    if (atoms && atoms.length > 1) {
      out += `${mark}{${mf.getValue(model.offsetOf(atoms[0]), model.offsetOf(atoms[atoms.length - 1]), "latex")}}`;
    }
  }
  return out;
}

/** The caret at one end of a cell of `atom` (a matrix, or a bracket group's
 *  one cell). A placeholder there is selected, as MathLive's own moves into
 *  a cell do, so typing replaces it. */
function placeCaret(mf: MathfieldElement, model: MlModel, atom: MlAtom, { cell: [r, c], at }: CellTarget) {
  const cell = atom.type === "leftright" ? atom.body : atom.rows?.[r]?.[c];
  if (!cell?.length) return;
  const pos = model.offsetOf(at === "start" ? cell[0] : cell[cell.length - 1]);
  if (model.at(pos)?.type === "placeholder") mf.selection = { ranges: [[pos - 1, pos]] };
  else if (model.at(pos)?.rightSibling?.type === "placeholder") mf.selection = { ranges: [[pos, pos + 1]] };
  else mf.position = pos;
}

/** Apply a grid edit: the structure is replaced whole by its new LaTeX (a
 *  one-cell grid as a bracket group, `toLatex`), then the caret goes to the
 *  target cell, or after the structure when it closed. */
export function applyGridEdit(mf: MathfieldElement, model: MlModel, { atom, group }: GridAt, edit: GridEdit) {
  if (edit.kind === "move") {
    if (!group) placeCaret(mf, model, atom, edit);
    return;
  }
  const left = atom.leftSibling;
  if (!left) return;
  const latex = toLatex(edit.grid, edit.kind === "close") + scripts(mf, model, atom);
  mf.selection = { ranges: [[model.offsetOf(left), model.offsetOf(atom)]] };
  mf.insert(latex, { format: "latex", mode: "math", selectionMode: "after" });
  // The atoms beside the structure survive; the new one follows `left`.
  const made = left.rightSibling;
  if (!made) return;
  // Inserted into an otherwise empty field, MathLive keeps the LaTeX verbatim
  // (`\right?`, placeholders) and copies that; a change to the atom drops it.
  made.isDirty = true;
  if (edit.kind === "close") mf.position = model.offsetOf(made);
  else placeCaret(mf, model, made, edit);
}

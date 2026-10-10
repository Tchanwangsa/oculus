import type { MathfieldElement } from "mathlive";

import { modelOf, type MlAtom, type MlModel } from "./model";

/** How far past its ink a slot (a script, a numerator) still takes a click. */
const SLOT_REACH = 4;

export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

const union = (a: Box, b: Box): Box => ({
  left: Math.min(a.left, b.left),
  top: Math.min(a.top, b.top),
  right: Math.max(a.right, b.right),
  bottom: Math.max(a.bottom, b.bottom),
});

interface Slot extends Box {
  root: boolean;
  /** A row of a root `lines` table (a block's top-level `\\` lines). */
  line: boolean;
  /** Each caret offset in the slot and the x it sits at. */
  carets: { offset: number; x: number }[];
}

/**
 * The caret offset for a click, or null to keep MathLive's. MathLive's
 * hit-test sends a click on an operator with scripts (`\cos^2`), or between
 * two atoms, to the front of the field. Here the click picks the innermost
 * slot (a script, a numerator, a cell, else the top level) whose box, a
 * little widened, holds it, and the caret gap in that slot nearest its x.
 * In a block of several lines, the row whose band holds the click (else the
 * nearest band) bounds that search, and a click in no slot of it, however
 * far beside it, takes the row's nearest gap — its start or end. A click
 * beside one-line maths, or inside a matrix but in no cell, keeps MathLive's
 * (its row logic, the field's two ends).
 */
export function caretAt(mf: MathfieldElement, x: number, y: number): number | null {
  const model = modelOf(mf);
  if (!model) return null;
  const slots = new Map<MlAtom, Map<string, Slot>>();
  const arrays: Box[] = [];
  for (let i = 0; i <= mf.lastOffset; i++) {
    const atom = model.at(i);
    const parent = atom?.parent;
    if (!atom || !parent) continue;
    // A bare `x^2` keeps its scripts in a box-less atom after the `x`, whose
    // caret sits past them; its slots came first, as children do.
    const own = [...(slots.get(atom)?.values() ?? [])];
    const r: Box | undefined = mf.getElementInfo(i)?.bounds ?? (own.length ? own.reduce<Box>(union, own[0]) : undefined);
    if (!r) continue;
    if (atom.type === "array") arrays.push(r);
    let captured = false;
    for (let a: MlAtom | undefined = parent; a; a = a.parent) if (a.captureSelection) captured = true;
    if (captured) continue;
    let byBranch = slots.get(parent);
    if (!byBranch) slots.set(parent, (byBranch = new Map()));
    const key = JSON.stringify(atom.parentBranch);
    // `union` copies: a DOMRect spreads to nothing.
    const prev = byBranch.get(key);
    const root = !parent.parent;
    const slot: Slot = { ...union(prev ?? r, r), root, line: root && parent.type === "array", carets: prev?.carets ?? [] };
    byBranch.set(key, slot);
    // A slot's leading `first` atom is the caret before its content.
    slot.carets.push({ offset: i, x: atom.type === "first" ? r.left : r.right });
  }
  let row: Slot | null = null;
  let gap = Infinity;
  for (const byBranch of slots.values()) {
    for (const s of byBranch.values()) {
      const d = y < s.top ? s.top - y : y > s.bottom ? y - s.bottom : 0;
      if (s.line && d < gap) [row, gap] = [s, d];
    }
  }
  let best: Slot | null = null;
  let area = Infinity;
  for (const byBranch of slots.values()) {
    for (const s of byBranch.values()) {
      // The top level takes any height, so the field's padding reaches it.
      const reach = s.root ? 0 : SLOT_REACH;
      if (x < s.left - reach || x > s.right + reach || (!s.root && (y < s.top || y > s.bottom))) continue;
      if (row && (s.line ? s !== row : s.bottom < row.top || s.top > row.bottom)) continue;
      const a = (s.right - s.left) * (s.bottom - s.top);
      if (a < area) [best, area] = [s, a];
    }
  }
  best ??= row;
  if (!best) return null;
  if (best.root && arrays.some((r) => x >= r.left && x <= r.right && y >= r.top && y <= r.bottom)) return null;
  let pick = best.carets[0];
  for (const c of best.carets) if (Math.abs(c.x - x) < Math.abs(pick.x - x)) pick = c;
  return pick.offset;
}

/**
 * A range whose ends sit at different depths, widened so both ends share a
 * branch and every structure it reaches into is taken whole. MathLive's
 * offsets put a matrix's cells before the matrix itself, so a drag from
 * beside one into a cell selects the cells but not the matrix. Ends in two
 * cells take the whole matrix. Null when the ends already share a branch.
 */
export function wholeStructures(model: MlModel, start: number, end: number): [number, number] | null {
  const chain = (atom: MlAtom | undefined) => {
    const out: MlAtom[] = [];
    for (let a = atom; a?.parent; a = a.parent) out.push(a);
    return out;
  };
  // The root `lines` table's rows read as one run, as the note's lines do.
  const branch = (a: MlAtom) => (a.parent?.environmentName === "lines" ? "lines" : JSON.stringify(a.parentBranch));
  const from = chain(model.at(start));
  const to = chain(model.at(end));
  for (const [i, a] of from.entries()) {
    const j = to.findIndex((b) => b.parent === a.parent && branch(b) === branch(a));
    if (j < 0) continue;
    if (i === 0 && j === 0) return null;
    // An offset is the caret after its atom: start before the lifted atom,
    // end after it.
    const left = i === 0 ? start : a.leftSibling && model.offsetOf(a.leftSibling);
    const right = j === 0 ? end : model.offsetOf(to[j]);
    return left == null || left < 0 || right < 0 ? null : [left, right];
  }
  return null;
}

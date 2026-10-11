import type { TableLayout } from "../tableModel";

/** The layout each table element currently shows. */
export const layouts = new WeakMap<HTMLElement, TableLayout>();

/** A block of selected cells: the one it started from and the one it reaches. */
export interface CellRange {
  ar: number;
  ac: number;
  hr: number;
  hc: number;
}

/** The block of cells each table element has selected, if any. */
export const ranges = new WeakMap<HTMLElement, CellRange>();

export function bounds(range: CellRange) {
  return {
    r0: Math.min(range.ar, range.hr),
    r1: Math.max(range.ar, range.hr),
    c0: Math.min(range.ac, range.hc),
    c1: Math.max(range.ac, range.hc),
  };
}

export interface Caret {
  node: Node;
  offset: number;
}

/** A cell element's value as one line: newlines become spaces. */
export function cellValue(el: HTMLElement): string {
  return (el.textContent ?? "").replace(/\s*\n\s*/g, " ").trim();
}

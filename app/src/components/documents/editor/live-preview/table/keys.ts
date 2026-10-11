import { redo, undo } from "@codemirror/commands";

import { caretAtEdge, cellAt, focusCell, onEdgeLine } from "./caret";
import { isCell, type TableCtx } from "./ctx";
import { addRow, leave } from "./edits";
import { rangeKeys, selectRange } from "./selection";
import { layouts } from "./state";

/** Keys inside a cell: Tab and Enter move between cells, arrows leave at the
 *  edges, Escape hands the caret back to the editor. */
export function cellKeydown(t: TableCtx, e: KeyboardEvent) {
  const { dom, view, scroll } = t;
  const cell = e.target;
  if (cell === scroll) return rangeKeys(t, e);
  if (!isCell(cell) || e.isComposing) return;
  const layout = layouts.get(dom);
  if (!layout) return;
  const r = Number(cell.dataset.r);
  const c = Number(cell.dataset.c);
  const rows = layout.rows.length;
  const cols = layout.align.length;
  const mod = e.metaKey || e.ctrlKey;

  if (mod && !e.altKey && e.key.toLowerCase() === "z") {
    e.preventDefault();
    if (e.shiftKey) redo(view);
    else undo(view);
  } else if (e.key === "Tab" && !mod && !e.altKey) {
    e.preventDefault();
    if (e.shiftKey) {
      const prev = c > 0 ? cellAt(dom, r, c - 1) : r > 0 ? cellAt(dom, r - 1, cols - 1) : null;
      if (prev) focusCell(prev);
    } else if (c + 1 < cols || r + 1 < rows) {
      const next = c + 1 < cols ? cellAt(dom, r, c + 1) : cellAt(dom, r + 1, 0);
      if (next) focusCell(next);
    } else {
      addRow(t, 0);
    }
  } else if (e.key === "Enter") {
    e.preventDefault();
    if (r + 1 < rows) {
      const below = cellAt(dom, r + 1, c);
      if (below) focusCell(below);
    } else {
      addRow(t, c);
    }
  } else if (e.key === "Escape") {
    e.preventDefault();
    leave(t);
  } else if ((e.key === "ArrowUp" || e.key === "ArrowDown") && !mod && !e.altKey && !e.shiftKey) {
    // Within a wrapped cell the browser moves between its lines.
    const up = e.key === "ArrowUp";
    if (!onEdgeLine(cell, up)) return;
    e.preventDefault();
    const to = up ? r - 1 : r + 1;
    const next = to >= 0 && to < rows ? cellAt(dom, to, c) : null;
    if (next) focusCell(next);
    else leave(t, up);
  } else if ((e.key === "ArrowUp" || e.key === "ArrowDown") && e.shiftKey && !mod && !e.altKey) {
    // Past the cell's edge line, Shift stretches a block into the next row.
    const up = e.key === "ArrowUp";
    const to = up ? r - 1 : r + 1;
    if (to < 0 || to >= rows || !onEdgeLine(cell, up)) return;
    e.preventDefault();
    selectRange(t, { ar: r, ac: c, hr: to, hc: c });
  } else if ((e.key === "ArrowLeft" || e.key === "ArrowRight") && !mod && !e.altKey && !e.shiftKey) {
    const left = e.key === "ArrowLeft";
    const edge = left ? r === 0 && c === 0 : r === rows - 1 && c === cols - 1;
    if (!edge || !caretAtEdge(cell, !left)) return;
    e.preventDefault();
    leave(t, left);
  }
}

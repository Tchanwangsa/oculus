import { redo, undo } from "@codemirror/commands";
import { copyText } from "@/lib/utils";

import { cellText, type TableLayout } from "../tableModel";
import { caretUnder, cellAt, focusCell } from "./caret";
import { place, type TableCtx } from "./ctx";
import { paintRange } from "./dom";
import { deleteRange } from "./edits";
import { bounds, layouts, ranges, type Caret, type CellRange } from "./state";

/**
 * A block of selected cells: dragging across cells, or Shift with a click or
 * ↑/↓, selects it; the grid's scroller then holds focus and takes the keys.
 * Delete empties the block, and on a block already empty removes the whole
 * rows or columns it spans.
 */

/** Select a block of cells; the scroller takes focus, and so the keys. */
export function selectRange(t: TableCtx, range: CellRange) {
  const { dom, scroll } = t;
  ranges.set(dom, range);
  paintRange(dom);
  if (dom.ownerDocument.activeElement !== scroll) scroll.focus({ preventScroll: true });
  dom.ownerDocument.getSelection()?.removeAllRanges();
  cellAt(dom, range.hr, range.hc)?.scrollIntoView({ block: "nearest", inline: "nearest" });
}

export function clearRange(t: TableCtx) {
  const { dom } = t;
  if (ranges.delete(dom)) paintRange(dom);
}

/** The selected block as tab-separated rows, as spreadsheets paste it. */
function rangeText(layout: TableLayout, range: CellRange) {
  const { r0, r1, c0, c1 } = bounds(range);
  const lines: string[] = [];
  for (let r = r0; r <= r1; r++) {
    const row: string[] = [];
    for (let c = c0; c <= c1; c++) row.push(cellText(layout, r, c).trim());
    lines.push(row.join("\t"));
  }
  return lines.join("\n");
}

const ARROWS: Record<string, [number, number]> = {
  ArrowUp: [-1, 0],
  ArrowDown: [1, 0],
  ArrowLeft: [0, -1],
  ArrowRight: [0, 1],
};

/** Keys while a block is selected. */
export function rangeKeys(t: TableCtx, e: KeyboardEvent) {
  const { dom, view } = t;
  const range = ranges.get(dom);
  const layout = layouts.get(dom);
  if (!range || !layout) return;
  const rows = layout.rows.length;
  const cols = layout.align.length;
  const mod = e.metaKey || e.ctrlKey;
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  const head = () => cellAt(dom, range.hr, range.hc);

  if (key === "Backspace" || key === "Delete") {
    e.preventDefault();
    deleteRange(t, true);
  } else if (mod && !e.altKey && key === "z") {
    e.preventDefault();
    if (e.shiftKey) redo(view);
    else undo(view);
  } else if (mod && !e.altKey && key === "a") {
    e.preventDefault();
    selectRange(t, { ar: 0, ac: 0, hr: rows - 1, hc: cols - 1 });
  } else if (mod && !e.altKey && (key === "c" || key === "x")) {
    e.preventDefault();
    void copyText(rangeText(layout, range));
    if (key === "x") deleteRange(t, false);
  } else if (key in ARROWS && !mod && !e.altKey) {
    e.preventDefault();
    const [dr, dc] = ARROWS[key];
    const hr = Math.max(0, Math.min(rows - 1, range.hr + dr));
    const hc = Math.max(0, Math.min(cols - 1, range.hc + dc));
    if (e.shiftKey) selectRange(t, { ...range, hr, hc });
    else {
      const cell = cellAt(dom, hr, hc);
      if (cell) focusCell(cell);
    }
  } else if (key === "Escape" || key === "Enter" || key === "Tab") {
    e.preventDefault();
    const cell = head();
    if (cell) focusCell(cell);
  } else if (e.key.length === 1 && !mod && !e.altKey) {
    // Typing over a block types over the cell it started from; the key
    // itself lands in the newly focused cell.
    const cell = cellAt(dom, range.ar, range.ac);
    if (cell) focusCell(cell, "all");
  }
}

/** Track a press from cell `ar`, `ac`. Within that cell it selects text
 *  from `caret`; once it reaches another cell it selects the block between
 *  them. The press's default is prevented, since WebKit's own drag would
 *  focus each cell it crosses, so the text selection is drawn here. */
export function dragRange(t: TableCtx, ar: number, ac: number, caret: Caret | null) {
  const { dom } = t;
  const doc = dom.ownerDocument;
  const move = (e: MouseEvent) => {
    e.preventDefault();
    const td = doc.elementFromPoint(e.clientX, e.clientY)?.closest("td, th");
    const cell = td && dom.contains(td) ? td.querySelector<HTMLElement>(".cm-table-cell") : null;
    if (!cell) return;
    const { r, c } = place(cell);
    const range = ranges.get(dom);
    if (!range && r === ar && c === ac) {
      const to = caret && caretUnder(cell, e.clientX, e.clientY);
      if (to) doc.getSelection()?.setBaseAndExtent(caret.node, caret.offset, to.node, to.offset);
    } else if (!range || range.hr !== r || range.hc !== c) {
      selectRange(t, { ar, ac, hr: r, hc: c });
    }
  };
  const up = () => {
    doc.removeEventListener("mousemove", move);
    doc.removeEventListener("mouseup", up);
  };
  doc.addEventListener("mousemove", move);
  doc.addEventListener("mouseup", up);
}

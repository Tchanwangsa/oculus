import { caretUnder, focusCell } from "./caret";
import { isCell, place, type TableCtx } from "./ctx";
import { dragRange, selectRange } from "./selection";
import { ranges } from "./state";

export function cellMousedown(t: TableCtx, e: MouseEvent) {
  const { dom } = t;
  const target = e.target as Element;
  if (target.closest(".cm-table-add, .cm-table-handle")) {
    // Keep focus where it is; the click does the work.
    e.preventDefault();
    return;
  }
  const td = target.closest("td, th");
  const cell = td?.querySelector<HTMLElement>(".cm-table-cell");
  if (!td || !cell || e.button !== 0) return;
  const { r, c } = place(cell);
  if (e.shiftKey) {
    // Shift-click stretches the block, or starts one at the focused cell.
    const range = ranges.get(dom);
    const active = dom.ownerDocument.activeElement;
    const from = range ? { r: range.ar, c: range.ac } : isCell(active) && dom.contains(active) ? place(active) : null;
    // Within the focused cell, Shift-click extends its text as usual.
    if (from && (range || from.r !== r || from.c !== c)) {
      e.preventDefault();
      selectRange(t, { ar: from.r, ac: from.c, hr: r, hc: c });
      dragRange(t, from.r, from.c, null);
      return;
    }
    if (from) return;
  }
  // Double and triple clicks select words and lines natively.
  if (e.detail > 1) return;
  e.preventDefault();
  // A press on a cell's padding, below a shorter neighbour's text, has no
  // caret under it: the caret goes to the end.
  const caret = caretUnder(cell, e.clientX, e.clientY);
  if (caret) {
    cell.focus({ preventScroll: true });
    const sel = dom.ownerDocument.getSelection();
    sel?.setBaseAndExtent(caret.node, caret.offset, caret.node, caret.offset);
  } else {
    focusCell(cell);
  }
  dragRange(t, r, c, caret);
}

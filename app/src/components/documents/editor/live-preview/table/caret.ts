import { cellValue, type Caret } from "./state";

export function cellAt(dom: HTMLElement, r: number, c: number): HTMLElement | null {
  return dom.querySelector<HTMLElement>(`.cm-table-cell[data-r="${r}"][data-c="${c}"]`);
}

/** The caret's character offset into a focused cell. */
export function caretIn(cell: HTMLElement): number {
  const sel = cell.ownerDocument.getSelection();
  if (!sel || !sel.rangeCount || !cell.contains(sel.focusNode)) return cellValue(cell).length;
  const range = document.createRange();
  range.selectNodeContents(cell);
  range.setEnd(sel.focusNode!, sel.focusOffset);
  return range.toString().length;
}

/** Whether a collapsed caret sits at the cell's start, or its end. */
export function caretAtEdge(cell: HTMLElement, end: boolean): boolean {
  if (!cell.ownerDocument.getSelection()?.isCollapsed) return false;
  return caretIn(cell) === (end ? (cell.textContent ?? "").length : 0);
}

/** Whether the caret is on the cell's first (`up`) or last visual line, so a
 *  vertical arrow leaves the cell. A caret WebKit cannot measure counts only
 *  at the text's start or end. */
export function onEdgeLine(cell: HTMLElement, up: boolean): boolean {
  const all = document.createRange();
  all.selectNodeContents(cell);
  const lines = [...all.getClientRects()].filter((r) => r.height > 0);
  if (!lines.length) return true;
  const top = Math.min(...lines.map((r) => r.top));
  const bottom = Math.max(...lines.map((r) => r.bottom));
  const height = Math.min(...lines.map((r) => r.height));
  if (bottom - top < height * 1.5) return true;
  const sel = cell.ownerDocument.getSelection();
  if (!sel?.rangeCount || !cell.contains(sel.focusNode)) return true;
  const caret = document.createRange();
  caret.setStart(sel.focusNode!, sel.focusOffset);
  const rect = caret.getBoundingClientRect();
  if (!rect.height) return caretAtEdge(cell, !up);
  const mid = (rect.top + rect.bottom) / 2;
  return up ? mid < top + height : mid > bottom - height;
}

/** Focus a cell with the caret at `offset` (its end by default), or with its
 *  text selected. */
export function focusCell(cell: HTMLElement, at: number | "end" | "all" = "end") {
  cell.focus({ preventScroll: true });
  const sel = cell.ownerDocument.getSelection();
  if (!sel) return;
  const range = document.createRange();
  const text = cell.firstChild;
  if (at === "all" || !text) {
    range.selectNodeContents(cell);
    if (at !== "all") range.collapse(false);
  } else {
    const len = text.textContent?.length ?? 0;
    const offset = at === "end" ? len : Math.min(at, len);
    range.setStart(text, offset);
    range.collapse(true);
  }
  sel.removeAllRanges();
  sel.addRange(range);
  cell.scrollIntoView({ block: "nearest", inline: "nearest" });
}

/** The caret position under a point, when it falls inside `cell`. */
export function caretUnder(cell: HTMLElement, x: number, y: number): Caret | null {
  const range = cell.ownerDocument.caretRangeFromPoint?.(x, y);
  return range && cell.contains(range.startContainer)
    ? { node: range.startContainer, offset: range.startOffset }
    : null;
}

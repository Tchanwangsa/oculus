import { appendCell, cellChange, removeCells, type Insert } from "../tableModel";
import { caretIn, cellAt, focusCell } from "./caret";
import { current, dispatch, goTo, type TableCtx } from "./ctx";
import { tableAt } from "./dom";
import { bounds, cellValue, ranges } from "./state";

export function write(t: TableCtx, cell: HTMLElement) {
  const { view } = t;
  const now = current(t);
  if (!now) return;
  const r = Number(cell.dataset.r);
  const c = Number(cell.dataset.c);
  const change = cellChange(now.layout, now.base, r, c, cellValue(cell));
  if (!change) return;
  const at = caretIn(cell);
  dispatch(t, change);
  // `updateDOM` keeps the element; should the view redraw it, follow.
  if (!cell.isConnected) {
    const el = tableAt(view, now.base);
    const next = el && cellAt(el, r, c);
    if (next) focusCell(next, at);
  }
}

export function addRow(t: TableCtx, col: number) {
  const now = current(t);
  if (!now) return;
  const at = now.base + now.layout.length;
  dispatch(t, { from: at, to: at, insert: `\n|${"   |".repeat(now.layout.align.length)}` }, "input");
  goTo(t, now.base, now.layout.rows.length, col);
}

export function addColumn(t: TableCtx) {
  const now = current(t);
  if (!now) return;
  const { layout, base } = now;
  const c = layout.align.length;
  const changes = [
    appendCell(layout.rows[0], base + layout.rows[0].at, c, `Column ${c + 1}`),
    appendCell(layout.delimiter, base + layout.delimiter.at, c, "---"),
    ...layout.rows.slice(1).map((row) => appendCell(row, base + row.at, c, "")),
  ].filter((x): x is Insert => x != null);
  dispatch(t, changes, "input");
  goTo(t, base, 0, c, "all");
}

/** Back to the editor, caret at the start of the line after the table or,
 *  going up, the end of the line before; a missing line is added. */
export function leave(t: TableCtx, up = false) {
  const { dom, view } = t;
  const now = current(t);
  if (!now) return;
  const start = now.base;
  const end = now.base + now.layout.length;
  const { doc } = view.state;
  (dom.ownerDocument.activeElement as HTMLElement | null)?.blur();
  if (up && start > 0) view.dispatch({ selection: { anchor: start - 1 }, scrollIntoView: true });
  else if (up) view.dispatch({ changes: { from: 0, insert: "\n" }, selection: { anchor: 0 }, userEvent: "input" });
  else if (end < doc.length) view.dispatch({ selection: { anchor: end + 1 }, scrollIntoView: true });
  else view.dispatch({ changes: { from: end, insert: "\n" }, selection: { anchor: end + 1 }, userEvent: "input" });
  view.focus();
}

/** The whole table out of the document, with its line break. */
function removeTable(t: TableCtx) {
  const { dom, view, scroll } = t;
  const now = current(t);
  if (!now) return;
  const from = now.base;
  const end = now.base + now.layout.length;
  const to = end < view.state.doc.length ? end + 1 : end;
  ranges.delete(dom);
  scroll.blur();
  view.dispatch({ changes: { from, to }, selection: { anchor: from }, userEvent: "delete", scrollIntoView: true });
  view.focus();
}

/** Delete on a block: empty its cells, or when they are already empty (and
 *  `structural`), remove the whole rows or columns it spans — the table
 *  itself when it spans every cell. */
export function deleteRange(t: TableCtx, structural: boolean) {
  const { dom } = t;
  const now = current(t);
  const range = ranges.get(dom);
  if (!now || !range) return;
  const { layout, base } = now;
  const { r0, r1, c0, c1 } = bounds(range);
  const rows = layout.rows.length;
  const cols = layout.align.length;

  const empties: Insert[] = [];
  for (let r = r0; r <= r1; r++) {
    for (let c = c0; c <= c1; c++) {
      const change = cellChange(layout, base, r, c, "");
      if (change) empties.push(change);
    }
  }
  if (empties.length) {
    dispatch(t, empties, "delete");
    return;
  }
  if (!structural) return;

  const wholeRows = c0 === 0 && c1 === cols - 1;
  const wholeCols = r0 === 0 && r1 === rows - 1;
  if (wholeRows && wholeCols) {
    removeTable(t);
  } else if (wholeRows) {
    // The header stays: a table cannot go without one.
    const first = Math.max(r0, 1);
    if (first > r1) return;
    const last = layout.rows[r1];
    ranges.delete(dom);
    dispatch(t, { from: base + layout.rows[first].at - 1, to: base + last.at + last.text.length, insert: "" }, "delete");
    goTo(t, base, Math.min(first, rows - 1 - (r1 - first + 1)), c0);
  } else if (wholeCols) {
    const changes = [layout.rows[0], layout.delimiter, ...layout.rows.slice(1)]
      .map((row) => removeCells(row, base + row.at, c0, c1))
      .filter((x): x is Insert => x != null);
    ranges.delete(dom);
    dispatch(t, changes, "delete");
    goTo(t, base, 0, Math.min(c0, cols - 1 - (c1 - c0 + 1)));
  }
}

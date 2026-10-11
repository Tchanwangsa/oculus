import { movedColumns, movedRows } from "../tableModel";
import { current, dispatch, place, type TableCtx } from "./ctx";
import { placeHandles, tableAt } from "./dom";
import { selectRange } from "./selection";
import { layouts, type CellRange } from "./state";

/** Point the handles at the row and column of `cell`. */
export function aim(t: TableCtx, cell: HTMLElement) {
  const { dom } = t;
  if (dom.classList.contains("cm-table-moving")) return;
  const { r, c } = place(cell);
  if (dom.dataset.hr === String(r) && dom.dataset.hc === String(c)) return;
  dom.dataset.hr = String(r);
  dom.dataset.hc = String(c);
  placeHandles(dom);
}

/** The gap a dragged row or column would drop into: the index it would sit
 *  before. Rows never go above the header. */
function gapAt(t: TableCtx, axis: "row" | "col", x: number, y: number): number {
  const { dom, scroll } = t;
  const table = scroll.querySelector("table");
  const layout = layouts.get(dom);
  if (!table || !layout) return -1;
  if (axis === "row") {
    for (let i = 1; i < table.rows.length; i++) {
      const rect = table.rows[i].getBoundingClientRect();
      if (y < rect.top + rect.height / 2) return i;
    }
    return table.rows.length;
  }
  const cells = table.rows[0].cells;
  for (let i = 0; i < cells.length; i++) {
    const rect = cells[i].getBoundingClientRect();
    if (x < rect.left + rect.width / 2) return i;
  }
  return cells.length;
}

/** Draw the drop line in gap `at`. */
function showDrop(t: TableCtx, axis: "row" | "col", at: number) {
  const { scroll, drop } = t;
  const table = scroll.querySelector("table");
  if (!table) return;
  drop.hidden = false;
  drop.dataset.axis = axis;
  if (axis === "row") {
    const y = at < table.rows.length ? table.rows[at].offsetTop : table.offsetHeight;
    drop.style.left = `${table.offsetLeft}px`;
    drop.style.top = `${table.offsetTop + y}px`;
    drop.style.width = `${scroll.clientWidth}px`;
    drop.style.height = "";
  } else {
    const cells = table.rows[0].cells;
    const x = at < cells.length ? cells[at].offsetLeft : table.offsetWidth;
    const left = Math.max(0, Math.min(scroll.clientWidth, x - scroll.scrollLeft));
    drop.style.left = `${table.offsetLeft + left}px`;
    drop.style.top = `${table.offsetTop}px`;
    drop.style.height = `${table.offsetHeight}px`;
    drop.style.width = "";
  }
}

/** Move row or column `from` into gap `to`, then select it there. */
function moveLine(t: TableCtx, axis: "row" | "col", from: number, to: number) {
  const { dom, view } = t;
  const now = current(t);
  if (!now || to === from || to === from + 1) return;
  const { layout, base } = now;
  if (axis === "row") {
    const start = layout.rows[1].at;
    const text = movedRows(layout, from, to);
    dispatch(t, { from: base + start, to: base + layout.length, insert: text.slice(start) }, "input");
  } else {
    dispatch(t, { from: base, to: base + layout.length, insert: movedColumns(layout, from, to) }, "input");
  }
  const at = to > from ? to - 1 : to;
  const next = layouts.get(tableAt(view, base) ?? dom);
  if (!next) return;
  if (axis === "row") selectRange(t, { ar: at, ac: 0, hr: at, hc: next.align.length - 1 });
  else selectRange(t, { ar: 0, ac: at, hr: next.rows.length - 1, hc: at });
}

/** A press on a handle: a click selects its row or column, a drag past the
 *  threshold moves it. Capture waits for the threshold, and the press's
 *  default is held off in `mousedown`, so focus stays where it is. */
export function handlePress(t: TableCtx, e: PointerEvent, axis: "row" | "col") {
  const { dom, drop } = t;
  const layout = layouts.get(dom);
  if (e.button !== 0 || !layout) return;
  const handle = e.currentTarget as HTMLElement;
  const rows = layout.rows.length;
  const cols = layout.align.length;
  const index = Math.min(Number((axis === "row" ? dom.dataset.hr : dom.dataset.hc) ?? 0), (axis === "row" ? rows : cols) - 1);
  const whole: CellRange = axis === "row"
    ? { ar: index, ac: 0, hr: index, hc: cols - 1 }
    : { ar: 0, ac: index, hr: rows - 1, hc: index };
  const movable = axis === "col" ? cols > 1 : index > 0 && rows > 2;
  const { pointerId, clientX: x0, clientY: y0 } = e;
  let lifted = false;
  let gap = -1;

  const onMove = (ev: PointerEvent) => {
    if (ev.pointerId !== pointerId) return;
    ev.preventDefault();
    if (!lifted) {
      if (!movable || Math.hypot(ev.clientX - x0, ev.clientY - y0) < 4) return;
      lifted = true;
      handle.setPointerCapture(pointerId);
      dom.classList.add("cm-table-moving");
      selectRange(t, whole);
    }
    gap = gapAt(t, axis, ev.clientX, ev.clientY);
    if (gap >= 0) showDrop(t, axis, gap);
  };
  const end = (commit: boolean) => (ev: PointerEvent) => {
    if (ev.pointerId !== pointerId) return;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("pointercancel", cancel);
    drop.hidden = true;
    dom.classList.remove("cm-table-moving");
    if (!commit) return;
    if (!lifted) selectRange(t, whole);
    else if (gap >= 0) moveLine(t, axis, index, gap);
  };
  const up = end(true);
  const cancel = end(false);
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", up);
  window.addEventListener("pointercancel", cancel);
}

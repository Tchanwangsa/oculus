import type { EditorView } from "@codemirror/view";

import type { Insert } from "../tableModel";
import { cellAt, focusCell } from "./caret";
import { tableAt } from "./dom";
import { layouts } from "./state";

/** What each table element's handlers close over. */
export interface TableCtx {
  dom: HTMLElement;
  view: EditorView;
  scroll: HTMLElement;
  drop: HTMLElement;
}

export function current(t: TableCtx) {
  const { dom, view } = t;
  const layout = layouts.get(dom);
  return layout ? { layout, base: view.posAtDOM(dom) } : null;
}

/** One edit; `input.type` lets history group typing as it does in text. */
export function dispatch(t: TableCtx, change: Insert | Insert[], userEvent = "input.type") {
  const { view } = t;
  const now = current(t);
  const changes = view.state.changes(change);
  view.dispatch({
    changes,
    // At the table's end, so undo scrolls here rather than to an old caret
    // and the caret never rests inside the rows' source.
    selection: view.hasFocus || !now ? undefined : { anchor: changes.mapPos(now.base + now.layout.length, 1) },
    userEvent,
  });
  view.requestMeasure();
}

/** Run after a structural change: focus cell `r`, `c` of the new grid. */
export function goTo(t: TableCtx, base: number, r: number, c: number, at: "end" | "all" = "end") {
  const { dom, view } = t;
  const el = tableAt(view, base) ?? dom;
  const cell = cellAt(el, r, c);
  if (cell) focusCell(cell, at);
}

export const isCell = (t: EventTarget | null): t is HTMLElement =>
  t instanceof HTMLElement && t.classList.contains("cm-table-cell");

export const place = (cell: HTMLElement) => ({ r: Number(cell.dataset.r), c: Number(cell.dataset.c) });

import { WidgetType, type EditorView } from "@codemirror/view";

import type { TableLayout } from "../tableModel";
import { attach } from "./attach";
import { cellAt, focusCell } from "./caret";
import { render, tableAt, tableButton } from "./dom";
import { layouts } from "./state";

/**
 * A GFM pipe table drawn as a grid whose cells edit in place. A cell writes
 * back by replacing only its own source range, so the rest of the table stays
 * byte-identical; adding a row or column appends pipe text.
 *
 * Any edit rebuilds the block field, and a widget that is not `eq` would get
 * fresh DOM, dropping the caret in the focused cell. So the widget compares by
 * source and `updateDOM` patches cell texts in place, leaving the focused cell
 * alone while it already reads right. Handlers never trust the widget that
 * drew them: the table's position comes from `posAtDOM` and its layout from
 * `layouts`, which `updateDOM` keeps current.
 *
 * Rows are split here rather than read from `TableCell` nodes, which lezer
 * omits for an empty cell; the split mirrors lezer's own.
 */

/** Focus a cell of the table drawn at `base` from the editor: in the header or
 *  the last row, the first or last column or the one nearest `x`, caret at the
 *  cell's start or end. False when the table is not drawn. */
export function enterTable(
  view: EditorView,
  base: number,
  row: "first" | "last",
  col: "first" | "last" | { x: number },
  at: "start" | "end",
): boolean {
  const el = tableAt(view, base);
  const layout = el && layouts.get(el);
  if (!el || !layout) return false;
  const r = row === "first" ? 0 : layout.rows.length - 1;
  const cols = layout.align.length;
  let c = col === "first" ? 0 : cols - 1;
  if (typeof col === "object") {
    let best = Infinity;
    for (let i = 0; i < cols; i++) {
      const rect = cellAt(el, r, i)?.parentElement?.getBoundingClientRect();
      if (!rect) continue;
      const d = Math.max(rect.left - col.x, col.x - rect.right, 0);
      if (d < best) {
        best = d;
        c = i;
      }
    }
  }
  const cell = cellAt(el, r, c);
  if (!cell) return false;
  focusCell(cell, at === "start" ? 0 : "end");
  return true;
}

export class TableWidget extends WidgetType {
  constructor(
    readonly source: string,
    readonly layout: TableLayout,
  ) {
    super();
  }

  eq(other: TableWidget) {
    return other.source === this.source;
  }

  toDOM(view: EditorView) {
    const dom = document.createElement("div");
    dom.className = "cm-table";
    const frame = document.createElement("div");
    frame.className = "cm-table-frame";
    const scroll = document.createElement("div");
    scroll.className = "cm-table-scroll";
    // Holds focus while a block of cells is selected.
    scroll.tabIndex = -1;
    const drop = document.createElement("div");
    drop.className = "cm-table-drop";
    drop.hidden = true;
    frame.append(
      scroll,
      tableButton("add", "cm-table-add-col", "Add column"),
      tableButton("add", "cm-table-add-row", "Add row"),
      tableButton("handle", "cm-table-handle-col", "Select column, or drag to move it"),
      tableButton("handle", "cm-table-handle-row", "Select row, or drag to move it"),
      drop,
    );
    dom.appendChild(frame);
    render(dom, this.layout);
    attach(dom, view);
    return dom;
  }

  updateDOM(dom: HTMLElement, view: EditorView) {
    if (!dom.classList.contains("cm-table")) return false;
    render(dom, this.layout);
    view.requestMeasure();
    return true;
  }

  /** Cells take their own clicks and keys. */
  ignoreEvent() {
    return true;
  }

  get estimatedHeight() {
    return this.layout.rows.length * 38 + 36;
  }
}

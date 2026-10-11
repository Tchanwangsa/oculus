import type { EditorView } from "@codemirror/view";

import { cellText, type TableLayout } from "../tableModel";
import { caretIn, cellAt, focusCell } from "./caret";
import { bounds, cellValue, layouts, ranges } from "./state";

/** Mark the selected cells, or none. */
export function paintRange(dom: HTMLElement) {
  const range = ranges.get(dom);
  const b = range && bounds(range);
  dom.classList.toggle("cm-table-ranged", !!range);
  for (const cell of dom.querySelectorAll<HTMLElement>(".cm-table-cell")) {
    const r = Number(cell.dataset.r);
    const c = Number(cell.dataset.c);
    const on = !!b && r >= b.r0 && r <= b.r1 && c >= b.c0 && c <= b.c1;
    cell.parentElement?.classList.toggle("cm-table-selected", on);
  }
  placeHandles(dom);
}

/** Put the row and column handles on the edges of the row and column of
 *  `dataset.hr`/`hc` — the cell last under the pointer or focused — lit when
 *  that whole row or column is the selected block. Offsets, not client rects,
 *  since page zoom scales the latter. */
export function placeHandles(dom: HTMLElement) {
  const layout = layouts.get(dom);
  const scroll = dom.querySelector<HTMLElement>(".cm-table-scroll");
  const table = scroll?.querySelector("table");
  const colHandle = dom.querySelector<HTMLElement>(".cm-table-handle-col");
  const rowHandle = dom.querySelector<HTMLElement>(".cm-table-handle-row");
  if (!layout || !scroll || !table || !colHandle || !rowHandle) return;
  const rows = layout.rows.length;
  const cols = layout.align.length;
  const r = Math.min(Number(dom.dataset.hr ?? 0), rows - 1);
  const c = Math.min(Number(dom.dataset.hc ?? 0), cols - 1);
  const td = table.rows[r]?.cells[c];
  if (!td) return;
  const tr = table.rows[r];

  const mid = td.offsetLeft + td.offsetWidth / 2 - scroll.scrollLeft;
  colHandle.hidden = mid < 0 || mid > scroll.clientWidth;
  colHandle.style.left = `${table.offsetLeft + mid}px`;
  colHandle.style.top = `${table.offsetTop}px`;
  rowHandle.style.left = `${table.offsetLeft}px`;
  rowHandle.style.top = `${table.offsetTop + tr.offsetTop + tr.offsetHeight / 2}px`;

  const range = ranges.get(dom);
  const b = range && bounds(range);
  colHandle.classList.toggle("cm-table-handle-on", !!b && b.c0 === c && b.c1 === c && b.r0 === 0 && b.r1 === rows - 1);
  rowHandle.classList.toggle("cm-table-handle-on", !!b && b.r0 === r && b.r1 === r && b.c0 === 0 && b.c1 === cols - 1);
}

function makeEditable(el: HTMLElement) {
  try {
    el.contentEditable = "plaintext-only";
  } catch {
    el.contentEditable = "true";
  }
}

function buildTable(layout: TableLayout): HTMLTableElement {
  const table = document.createElement("table");
  const head = table.createTHead();
  const body = table.createTBody();
  layout.rows.forEach((_, r) => {
    const tr = (r === 0 ? head : body).insertRow();
    layout.align.forEach((align, c) => {
      const td = document.createElement(r === 0 ? "th" : "td");
      const cell = document.createElement("div");
      cell.className = "cm-table-cell";
      makeEditable(cell);
      cell.spellcheck = true;
      cell.dataset.r = String(r);
      cell.dataset.c = String(c);
      cell.style.textAlign = align ?? "";
      cell.textContent = cellText(layout, r, c);
      td.appendChild(cell);
      tr.appendChild(td);
    });
  });
  return table;
}

/** Show `layout` in a table element: patch texts when the shape holds,
 *  otherwise rebuild the grid and put the caret back where it was. */
export function render(dom: HTMLElement, layout: TableLayout) {
  layouts.set(dom, layout);
  const scroll = dom.querySelector<HTMLElement>(".cm-table-scroll");
  if (!scroll) return;
  const shape = `${layout.rows.length}x${layout.align.length}`;
  const active = dom.ownerDocument.activeElement;
  const focused = active instanceof HTMLElement && active.classList.contains("cm-table-cell") && dom.contains(active)
    ? active
    : null;

  if (scroll.dataset.shape !== shape || !scroll.firstChild) {
    const was = focused ? { r: Number(focused.dataset.r), c: Number(focused.dataset.c), at: caretIn(focused) } : null;
    scroll.replaceChildren(buildTable(layout));
    scroll.dataset.shape = shape;
    const range = ranges.get(dom);
    if (range) {
      const r = layout.rows.length - 1;
      const c = layout.align.length - 1;
      ranges.set(dom, {
        ar: Math.min(range.ar, r),
        ac: Math.min(range.ac, c),
        hr: Math.min(range.hr, r),
        hc: Math.min(range.hc, c),
      });
    }
    paintRange(dom);
    if (was) {
      const r = Math.min(was.r, layout.rows.length - 1);
      const c = Math.min(was.c, layout.align.length - 1);
      const cell = cellAt(dom, r, c);
      if (cell) focusCell(cell, r === was.r && c === was.c ? was.at : "end");
    }
    return;
  }

  for (const cell of scroll.querySelectorAll<HTMLElement>(".cm-table-cell")) {
    const r = Number(cell.dataset.r);
    const c = Number(cell.dataset.c);
    const text = cellText(layout, r, c);
    cell.style.textAlign = layout.align[c] ?? "";
    if (cell === focused) {
      // The edit came from here, so it already reads right; an undo may not.
      if (cellValue(cell) !== text.trim()) {
        cell.textContent = text;
        focusCell(cell);
      }
    } else if (cell.textContent !== text) {
      cell.textContent = text;
    }
  }
  placeHandles(dom);
}

/** The table element drawn at `base` after an update, which may be new. */
export function tableAt(view: EditorView, base: number): HTMLElement | null {
  for (const el of view.contentDOM.querySelectorAll<HTMLElement>(".cm-table")) {
    if (view.posAtDOM(el) === base) return el;
  }
  return null;
}

export function tableButton(kind: "handle" | "add", cls: string, label: string): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = `cm-table-${kind} ${cls}`;
  button.title = label;
  button.setAttribute("aria-label", label);
  if (kind === "add") button.textContent = "+";
  return button;
}

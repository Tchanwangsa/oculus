import { redo, undo } from "@codemirror/commands";
import { WidgetType, type EditorView } from "@codemirror/view";
import { copyText } from "@/lib/utils";

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
 *
 * Dragging across cells, or Shift with a click or ↑/↓, selects a block of
 * them; the grid's scroller then holds focus and takes the keys. Delete
 * empties the block, and on a block already empty removes the whole rows or
 * columns it spans. Handles on the top and left edges, for the row and column
 * under the pointer, select that row or column on a click and move it on a
 * drag; the header row stays put, since a body row may be short of cells.
 */

import {
  appendCell,
  cellChange,
  cellText,
  movedColumns,
  movedRows,
  removeCells,
  type Insert,
  type TableLayout,
} from "./tableModel";

/** A cell element's value as one line: newlines become spaces. */
function cellValue(el: HTMLElement): string {
  return (el.textContent ?? "").replace(/\s*\n\s*/g, " ").trim();
}

// ── DOM ─────────────────────────────────────────────────────────────────────

/** The layout each table element currently shows. */
const layouts = new WeakMap<HTMLElement, TableLayout>();

/** A block of selected cells: the one it started from and the one it reaches. */
interface CellRange {
  ar: number;
  ac: number;
  hr: number;
  hc: number;
}

/** The block of cells each table element has selected, if any. */
const ranges = new WeakMap<HTMLElement, CellRange>();

function bounds(range: CellRange) {
  return {
    r0: Math.min(range.ar, range.hr),
    r1: Math.max(range.ar, range.hr),
    c0: Math.min(range.ac, range.hc),
    c1: Math.max(range.ac, range.hc),
  };
}

/** Mark the selected cells, or none. */
function paintRange(dom: HTMLElement) {
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
function placeHandles(dom: HTMLElement) {
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

function cellAt(dom: HTMLElement, r: number, c: number): HTMLElement | null {
  return dom.querySelector<HTMLElement>(`.cm-table-cell[data-r="${r}"][data-c="${c}"]`);
}

/** The caret's character offset into a focused cell. */
function caretIn(cell: HTMLElement): number {
  const sel = cell.ownerDocument.getSelection();
  if (!sel || !sel.rangeCount || !cell.contains(sel.focusNode)) return cellValue(cell).length;
  const range = document.createRange();
  range.selectNodeContents(cell);
  range.setEnd(sel.focusNode!, sel.focusOffset);
  return range.toString().length;
}

/** Whether a collapsed caret sits at the cell's start, or its end. */
function caretAtEdge(cell: HTMLElement, end: boolean): boolean {
  if (!cell.ownerDocument.getSelection()?.isCollapsed) return false;
  return caretIn(cell) === (end ? (cell.textContent ?? "").length : 0);
}

/** Whether the caret is on the cell's first (`up`) or last visual line, so a
 *  vertical arrow leaves the cell. A caret WebKit cannot measure counts only
 *  at the text's start or end. */
function onEdgeLine(cell: HTMLElement, up: boolean): boolean {
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
function focusCell(cell: HTMLElement, at: number | "end" | "all" = "end") {
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

interface Caret {
  node: Node;
  offset: number;
}

/** The caret position under a point, when it falls inside `cell`. */
function caretUnder(cell: HTMLElement, x: number, y: number): Caret | null {
  const range = cell.ownerDocument.caretRangeFromPoint?.(x, y);
  return range && cell.contains(range.startContainer)
    ? { node: range.startContainer, offset: range.startOffset }
    : null;
}

/** Show `layout` in a table element: patch texts when the shape holds,
 *  otherwise rebuild the grid and put the caret back where it was. */
function render(dom: HTMLElement, layout: TableLayout) {
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
function tableAt(view: EditorView, base: number): HTMLElement | null {
  for (const el of view.contentDOM.querySelectorAll<HTMLElement>(".cm-table")) {
    if (view.posAtDOM(el) === base) return el;
  }
  return null;
}

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

/** Wires one table element's events. Everything is delegated from the root,
 *  which outlives the grid inside it. */
function attach(dom: HTMLElement, view: EditorView) {
  const scroll = dom.querySelector<HTMLElement>(".cm-table-scroll")!;
  const current = () => {
    const layout = layouts.get(dom);
    return layout ? { layout, base: view.posAtDOM(dom) } : null;
  };

  /** One edit; `input.type` lets history group typing as it does in text. */
  const dispatch = (change: Insert | Insert[], userEvent = "input.type") => {
    const now = current();
    const changes = view.state.changes(change);
    view.dispatch({
      changes,
      // At the table's end, so undo scrolls here rather than to an old caret
      // and the caret never rests inside the rows' source.
      selection: view.hasFocus || !now ? undefined : { anchor: changes.mapPos(now.base + now.layout.length, 1) },
      userEvent,
    });
    view.requestMeasure();
  };

  /** Run after a structural change: focus cell `r`, `c` of the new grid. */
  const goTo = (base: number, r: number, c: number, at: "end" | "all" = "end") => {
    const el = tableAt(view, base) ?? dom;
    const cell = cellAt(el, r, c);
    if (cell) focusCell(cell, at);
  };

  const write = (cell: HTMLElement) => {
    const now = current();
    if (!now) return;
    const r = Number(cell.dataset.r);
    const c = Number(cell.dataset.c);
    const change = cellChange(now.layout, now.base, r, c, cellValue(cell));
    if (!change) return;
    const at = caretIn(cell);
    dispatch(change);
    // `updateDOM` keeps the element; should the view redraw it, follow.
    if (!cell.isConnected) {
      const el = tableAt(view, now.base);
      const next = el && cellAt(el, r, c);
      if (next) focusCell(next, at);
    }
  };

  const addRow = (col: number) => {
    const now = current();
    if (!now) return;
    const at = now.base + now.layout.length;
    dispatch({ from: at, to: at, insert: `\n|${"   |".repeat(now.layout.align.length)}` }, "input");
    goTo(now.base, now.layout.rows.length, col);
  };

  const addColumn = () => {
    const now = current();
    if (!now) return;
    const { layout, base } = now;
    const c = layout.align.length;
    const changes = [
      appendCell(layout.rows[0], base + layout.rows[0].at, c, `Column ${c + 1}`),
      appendCell(layout.delimiter, base + layout.delimiter.at, c, "---"),
      ...layout.rows.slice(1).map((row) => appendCell(row, base + row.at, c, "")),
    ].filter((x): x is Insert => x != null);
    dispatch(changes, "input");
    goTo(base, 0, c, "all");
  };

  /** Back to the editor, caret at the start of the line after the table or,
   *  going up, the end of the line before; a missing line is added. */
  const leave = (up = false) => {
    const now = current();
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
  };

  const isCell = (t: EventTarget | null): t is HTMLElement =>
    t instanceof HTMLElement && t.classList.contains("cm-table-cell");

  const place = (cell: HTMLElement) => ({ r: Number(cell.dataset.r), c: Number(cell.dataset.c) });

  /** Select a block of cells; the scroller takes focus, and so the keys. */
  const selectRange = (range: CellRange) => {
    ranges.set(dom, range);
    paintRange(dom);
    if (dom.ownerDocument.activeElement !== scroll) scroll.focus({ preventScroll: true });
    dom.ownerDocument.getSelection()?.removeAllRanges();
    cellAt(dom, range.hr, range.hc)?.scrollIntoView({ block: "nearest", inline: "nearest" });
  };

  const clearRange = () => {
    if (ranges.delete(dom)) paintRange(dom);
  };

  /** The selected block as tab-separated rows, as spreadsheets paste it. */
  const rangeText = (layout: TableLayout, range: CellRange) => {
    const { r0, r1, c0, c1 } = bounds(range);
    const lines: string[] = [];
    for (let r = r0; r <= r1; r++) {
      const row: string[] = [];
      for (let c = c0; c <= c1; c++) row.push(cellText(layout, r, c).trim());
      lines.push(row.join("\t"));
    }
    return lines.join("\n");
  };

  /** The whole table out of the document, with its line break. */
  const removeTable = () => {
    const now = current();
    if (!now) return;
    const from = now.base;
    const end = now.base + now.layout.length;
    const to = end < view.state.doc.length ? end + 1 : end;
    ranges.delete(dom);
    scroll.blur();
    view.dispatch({ changes: { from, to }, selection: { anchor: from }, userEvent: "delete", scrollIntoView: true });
    view.focus();
  };

  /** Delete on a block: empty its cells, or when they are already empty (and
   *  `structural`), remove the whole rows or columns it spans — the table
   *  itself when it spans every cell. */
  const deleteRange = (structural: boolean) => {
    const now = current();
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
      dispatch(empties, "delete");
      return;
    }
    if (!structural) return;

    const wholeRows = c0 === 0 && c1 === cols - 1;
    const wholeCols = r0 === 0 && r1 === rows - 1;
    if (wholeRows && wholeCols) {
      removeTable();
    } else if (wholeRows) {
      // The header stays: a table cannot go without one.
      const first = Math.max(r0, 1);
      if (first > r1) return;
      const last = layout.rows[r1];
      ranges.delete(dom);
      dispatch({ from: base + layout.rows[first].at - 1, to: base + last.at + last.text.length, insert: "" }, "delete");
      goTo(base, Math.min(first, rows - 1 - (r1 - first + 1)), c0);
    } else if (wholeCols) {
      const changes = [layout.rows[0], layout.delimiter, ...layout.rows.slice(1)]
        .map((row) => removeCells(row, base + row.at, c0, c1))
        .filter((x): x is Insert => x != null);
      ranges.delete(dom);
      dispatch(changes, "delete");
      goTo(base, 0, Math.min(c0, cols - 1 - (c1 - c0 + 1)));
    }
  };

  const ARROWS: Record<string, [number, number]> = {
    ArrowUp: [-1, 0],
    ArrowDown: [1, 0],
    ArrowLeft: [0, -1],
    ArrowRight: [0, 1],
  };

  /** Keys while a block is selected. */
  const rangeKeys = (e: KeyboardEvent) => {
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
      deleteRange(true);
    } else if (mod && !e.altKey && key === "z") {
      e.preventDefault();
      if (e.shiftKey) redo(view);
      else undo(view);
    } else if (mod && !e.altKey && key === "a") {
      e.preventDefault();
      selectRange({ ar: 0, ac: 0, hr: rows - 1, hc: cols - 1 });
    } else if (mod && !e.altKey && (key === "c" || key === "x")) {
      e.preventDefault();
      void copyText(rangeText(layout, range));
      if (key === "x") deleteRange(false);
    } else if (key in ARROWS && !mod && !e.altKey) {
      e.preventDefault();
      const [dr, dc] = ARROWS[key];
      const hr = Math.max(0, Math.min(rows - 1, range.hr + dr));
      const hc = Math.max(0, Math.min(cols - 1, range.hc + dc));
      if (e.shiftKey) selectRange({ ...range, hr, hc });
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
  };

  /** Track a press from cell `ar`, `ac`. Within that cell it selects text
   *  from `caret`; once it reaches another cell it selects the block between
   *  them. The press's default is prevented, since WebKit's own drag would
   *  focus each cell it crosses, so the text selection is drawn here. */
  const dragRange = (ar: number, ac: number, caret: Caret | null) => {
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
        selectRange({ ar, ac, hr: r, hc: c });
      }
    };
    const up = () => {
      doc.removeEventListener("mousemove", move);
      doc.removeEventListener("mouseup", up);
    };
    doc.addEventListener("mousemove", move);
    doc.addEventListener("mouseup", up);
  };

  // Editing a cell ends the block; so does focus leaving the table, but not
  // the window losing it.
  dom.addEventListener("focusin", (e) => {
    if (isCell(e.target)) clearRange();
  });
  dom.addEventListener("focusout", (e) => {
    const to = e.relatedTarget;
    if (to instanceof Node && dom.contains(to)) return;
    if (!to && !dom.ownerDocument.hasFocus()) return;
    clearRange();
  });

  dom.addEventListener("input", (e) => {
    if (isCell(e.target) && !(e as InputEvent).isComposing) write(e.target);
  });
  dom.addEventListener("compositionend", (e) => {
    if (isCell(e.target)) write(e.target);
  });

  dom.addEventListener("beforeinput", (e) => {
    if (!isCell(e.target)) return;
    // The cell's own undo stack knows nothing of the document's.
    if (e.inputType === "historyUndo" || e.inputType === "historyRedo") {
      e.preventDefault();
      if (e.inputType === "historyUndo") undo(view);
      else redo(view);
    } else if (e.inputType === "insertParagraph" || e.inputType === "insertLineBreak") {
      e.preventDefault();
    }
  });

  dom.addEventListener("paste", (e) => {
    if (!isCell(e.target)) return;
    e.preventDefault();
    const text = (e.clipboardData?.getData("text/plain") ?? "").replace(/\s*\r?\n\s*/g, " ");
    if (text) document.execCommand("insertText", false, text);
  });

  dom.addEventListener("keydown", (e) => {
    const cell = e.target;
    if (cell === scroll) return rangeKeys(e);
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
        addRow(0);
      }
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (r + 1 < rows) {
        const below = cellAt(dom, r + 1, c);
        if (below) focusCell(below);
      } else {
        addRow(c);
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      leave();
    } else if ((e.key === "ArrowUp" || e.key === "ArrowDown") && !mod && !e.altKey && !e.shiftKey) {
      // Within a wrapped cell the browser moves between its lines.
      const up = e.key === "ArrowUp";
      if (!onEdgeLine(cell, up)) return;
      e.preventDefault();
      const to = up ? r - 1 : r + 1;
      const next = to >= 0 && to < rows ? cellAt(dom, to, c) : null;
      if (next) focusCell(next);
      else leave(up);
    } else if ((e.key === "ArrowUp" || e.key === "ArrowDown") && e.shiftKey && !mod && !e.altKey) {
      // Past the cell's edge line, Shift stretches a block into the next row.
      const up = e.key === "ArrowUp";
      const to = up ? r - 1 : r + 1;
      if (to < 0 || to >= rows || !onEdgeLine(cell, up)) return;
      e.preventDefault();
      selectRange({ ar: r, ac: c, hr: to, hc: c });
    } else if ((e.key === "ArrowLeft" || e.key === "ArrowRight") && !mod && !e.altKey && !e.shiftKey) {
      const left = e.key === "ArrowLeft";
      const edge = left ? r === 0 && c === 0 : r === rows - 1 && c === cols - 1;
      if (!edge || !caretAtEdge(cell, !left)) return;
      e.preventDefault();
      leave(left);
    }
  });

  /** Point the handles at the row and column of `cell`. */
  const aim = (cell: HTMLElement) => {
    if (dom.classList.contains("cm-table-moving")) return;
    const { r, c } = place(cell);
    if (dom.dataset.hr === String(r) && dom.dataset.hc === String(c)) return;
    dom.dataset.hr = String(r);
    dom.dataset.hc = String(c);
    placeHandles(dom);
  };

  dom.addEventListener("pointerover", (e) => {
    const cell = (e.target as Element).closest("td, th")?.querySelector<HTMLElement>(".cm-table-cell");
    if (cell && dom.contains(cell)) aim(cell);
  });
  dom.addEventListener("focusin", (e) => {
    if (isCell(e.target)) aim(e.target);
  });
  scroll.addEventListener("scroll", () => placeHandles(dom));

  const drop = dom.querySelector<HTMLElement>(".cm-table-drop")!;

  /** The gap a dragged row or column would drop into: the index it would sit
   *  before. Rows never go above the header. */
  const gapAt = (axis: "row" | "col", x: number, y: number): number => {
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
  };

  /** Draw the drop line in gap `at`. */
  const showDrop = (axis: "row" | "col", at: number) => {
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
  };

  /** Move row or column `from` into gap `to`, then select it there. */
  const move = (axis: "row" | "col", from: number, to: number) => {
    const now = current();
    if (!now || to === from || to === from + 1) return;
    const { layout, base } = now;
    if (axis === "row") {
      const start = layout.rows[1].at;
      const text = movedRows(layout, from, to);
      dispatch({ from: base + start, to: base + layout.length, insert: text.slice(start) }, "input");
    } else {
      dispatch({ from: base, to: base + layout.length, insert: movedColumns(layout, from, to) }, "input");
    }
    const at = to > from ? to - 1 : to;
    const next = layouts.get(tableAt(view, base) ?? dom);
    if (!next) return;
    if (axis === "row") selectRange({ ar: at, ac: 0, hr: at, hc: next.align.length - 1 });
    else selectRange({ ar: 0, ac: at, hr: next.rows.length - 1, hc: at });
  };

  /** A press on a handle: a click selects its row or column, a drag past the
   *  threshold moves it. Capture waits for the threshold, and the press's
   *  default is held off in `mousedown`, so focus stays where it is. */
  const handlePress = (e: PointerEvent, axis: "row" | "col") => {
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
        selectRange(whole);
      }
      gap = gapAt(axis, ev.clientX, ev.clientY);
      if (gap >= 0) showDrop(axis, gap);
    };
    const end = (commit: boolean) => (ev: PointerEvent) => {
      if (ev.pointerId !== pointerId) return;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
      drop.hidden = true;
      dom.classList.remove("cm-table-moving");
      if (!commit) return;
      if (!lifted) selectRange(whole);
      else if (gap >= 0) move(axis, index, gap);
    };
    const up = end(true);
    const cancel = end(false);
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
  };

  dom.querySelector<HTMLElement>(".cm-table-handle-row")?.addEventListener("pointerdown", (e) => handlePress(e, "row"));
  dom.querySelector<HTMLElement>(".cm-table-handle-col")?.addEventListener("pointerdown", (e) => handlePress(e, "col"));

  dom.addEventListener("mousedown", (e) => {
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
        selectRange({ ar: from.r, ac: from.c, hr: r, hc: c });
        dragRange(from.r, from.c, null);
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
    dragRange(r, c, caret);
  });

  dom.querySelector(".cm-table-add-row")?.addEventListener("click", () => addRow(0));
  dom.querySelector(".cm-table-add-col")?.addEventListener("click", () => addColumn());
}

function tableButton(kind: "handle" | "add", cls: string, label: string): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = `cm-table-${kind} ${cls}`;
  button.title = label;
  button.setAttribute("aria-label", label);
  if (kind === "add") button.textContent = "+";
  return button;
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

import { redo, undo } from "@codemirror/commands";
import type { EditorView } from "@codemirror/view";

import { clipboardGrid } from "../tableGrid";
import { type TableCtx, isCell, place } from "./ctx";
import { placeHandles } from "./dom";
import { addColumn, addRow, pasteGrid, write as writeCell } from "./edits";
import { aim, handlePress } from "./handles";
import { cellKeydown } from "./keys";
import { cellMousedown } from "./press";
import { paintRich, showSource } from "./rich";
import { blockHeld, clearRange, copyBlock } from "./selection";
import { bounds, cellValue, ranges } from "./state";

/** Wires one table element's events. Everything is delegated from the root,
 *  which outlives the grid inside it. */
export function attach(dom: HTMLElement, view: EditorView) {
  const scroll = dom.querySelector<HTMLElement>(".cm-table-scroll")!;
  const drop = dom.querySelector<HTMLElement>(".cm-table-drop")!;
  const t: TableCtx = { dom, view, scroll, drop };

  // Editing a cell ends the block; so does focus leaving the table, but not
  // the window losing it.
  dom.addEventListener("focusin", (e) => {
    if (isCell(e.target)) clearRange(t);
  });
  dom.addEventListener("focusout", (e) => {
    const to = e.relatedTarget;
    if (to instanceof Node && dom.contains(to)) return;
    if (!to && !dom.ownerDocument.hasFocus()) return;
    clearRange(t);
  });

  dom.addEventListener("input", (e) => {
    if (isCell(e.target) && !(e as InputEvent).isComposing) writeCell(t, e.target);
  });
  dom.addEventListener("compositionend", (e) => {
    if (isCell(e.target)) writeCell(t, e.target);
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

  // WebKit leaves Copy, Cut and Paste off while nothing is selected, unless
  // the page claims them: a selected block of cells has no text selection.
  for (const type of ["beforecopy", "beforecut", "beforepaste"]) {
    dom.addEventListener(type, (e) => {
      if (blockHeld(t, e)) e.preventDefault();
    });
  }
  dom.addEventListener("copy", (e) => copyBlock(t, e, false));
  dom.addEventListener("cut", (e) => copyBlock(t, e, true));

  dom.addEventListener("paste", (e) => {
    const range = ranges.get(dom);
    const cell = isCell(e.target) ? e.target : null;
    if (!cell && !(range && e.target === scroll)) return;
    e.preventDefault();
    // A grid fills cells from this one, or from a selected block's corner.
    const grid = clipboardGrid(e.clipboardData, false);
    if (grid && (grid.length > 1 || grid[0].length > 1)) {
      const { r0, c0 } = range ? bounds(range) : { r0: place(cell!).r, c0: place(cell!).c };
      pasteGrid(t, r0, c0, grid);
      return;
    }
    const text = (e.clipboardData?.getData("text/plain") ?? "").replace(/\s*\r?\n\s*/g, " ");
    if (cell && text) document.execCommand("insertText", false, text);
  });

  dom.addEventListener("keydown", (e) => cellKeydown(t, e));

  dom.addEventListener("pointerover", (e) => {
    const cell = (e.target as Element).closest("td, th")?.querySelector<HTMLElement>(".cm-table-cell");
    if (cell && dom.contains(cell)) aim(t, cell);
  });
  dom.addEventListener("focusin", (e) => {
    if (!isCell(e.target)) return;
    showSource(e.target);
    aim(t, e.target);
  });
  dom.addEventListener("focusout", (e) => {
    if (isCell(e.target)) paintRich(e.target, cellValue(e.target), false);
  });
  scroll.addEventListener("scroll", () => placeHandles(dom));

  dom.querySelector<HTMLElement>(".cm-table-handle-row")?.addEventListener("pointerdown", (e) => handlePress(t, e, "row"));
  dom.querySelector<HTMLElement>(".cm-table-handle-col")?.addEventListener("pointerdown", (e) => handlePress(t, e, "col"));

  dom.addEventListener("mousedown", (e) => cellMousedown(t, e));

  dom.querySelector(".cm-table-add-row")?.addEventListener("click", () => addRow(t, 0));
  dom.querySelector(".cm-table-add-col")?.addEventListener("click", () => addColumn(t));
}

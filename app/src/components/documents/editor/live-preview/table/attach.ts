import { redo, undo } from "@codemirror/commands";
import type { EditorView } from "@codemirror/view";

import { type TableCtx, isCell } from "./ctx";
import { placeHandles } from "./dom";
import { addColumn, addRow, write as writeCell } from "./edits";
import { aim, handlePress } from "./handles";
import { cellKeydown } from "./keys";
import { cellMousedown } from "./press";
import { clearRange } from "./selection";

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

  dom.addEventListener("paste", (e) => {
    if (!isCell(e.target)) return;
    e.preventDefault();
    const text = (e.clipboardData?.getData("text/plain") ?? "").replace(/\s*\r?\n\s*/g, " ");
    if (text) document.execCommand("insertText", false, text);
  });

  dom.addEventListener("keydown", (e) => cellKeydown(t, e));

  dom.addEventListener("pointerover", (e) => {
    const cell = (e.target as Element).closest("td, th")?.querySelector<HTMLElement>(".cm-table-cell");
    if (cell && dom.contains(cell)) aim(t, cell);
  });
  dom.addEventListener("focusin", (e) => {
    if (isCell(e.target)) aim(t, e.target);
  });
  scroll.addEventListener("scroll", () => placeHandles(dom));

  dom.querySelector<HTMLElement>(".cm-table-handle-row")?.addEventListener("pointerdown", (e) => handlePress(t, e, "row"));
  dom.querySelector<HTMLElement>(".cm-table-handle-col")?.addEventListener("pointerdown", (e) => handlePress(t, e, "col"));

  dom.addEventListener("mousedown", (e) => cellMousedown(t, e));

  dom.querySelector(".cm-table-add-row")?.addEventListener("click", () => addRow(t, 0));
  dom.querySelector(".cm-table-add-col")?.addEventListener("click", () => addColumn(t));
}

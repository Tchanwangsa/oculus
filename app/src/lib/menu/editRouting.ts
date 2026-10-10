import type { MathfieldElement } from "mathlive";

import { selectAllMaths } from "@/lib/markdown/mathSelection";
import { OVERLAY, currentFindTarget, selectContents } from "@/lib/menu/find";

/**
 * What the Edit menu's Undo, Redo and Select All do (`shell/menu.rs` only emits
 * them while the app's webview has focus).
 *
 * Undo and Redo: a registered handler takes the step if it owns the focus (a
 * note editor keeps its own history), else the focused text control undoes
 * natively.
 *
 * Select All selects inside the screen in use, never the whole window: a
 * selection inside rendered maths widens in its formula
 * (`lib/markdown/mathSelection/`), else the focused control's own ⌘A (a note, a table's cell block, a maths field),
 * else a focused text control's contents, else the dialog or popover holding
 * focus, else the target ⌘F would search (`lib/menu/find.ts`).
 */

type EditHandler = (redo: boolean) => boolean;

const handlers = new Set<EditHandler>();

export const EDITABLE =
  "input, textarea, select, [contenteditable=''], [contenteditable='true'], [contenteditable='plaintext-only']";

/** Offer undo and redo to `handler` first; it returns whether it took it. */
export function onEdit(handler: EditHandler): () => void {
  handlers.add(handler);
  return () => void handlers.delete(handler);
}

export function routeEdit(redo: boolean): void {
  for (const handler of handlers) if (handler(redo)) return;
  const active = document.activeElement;
  if (active?.closest(EDITABLE)) document.execCommand(redo ? "redo" : "undo");
}

const MAC = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

/** Replays ⌘A on `el`, as the menu kept the real key from it; whether a
 *  handler (CodeMirror's keymap, `components/documents/editor/live-preview/table/`) took it. */
function offerKey(el: Element): boolean {
  const key = new KeyboardEvent("keydown", {
    key: "a",
    code: "KeyA",
    metaKey: MAC,
    ctrlKey: !MAC,
    bubbles: true,
    cancelable: true,
  });
  return !el.dispatchEvent(key);
}

/** Select All; `pane` is the active tab's focused pane. */
export function routeSelectAll(pane: number | undefined): void {
  if (selectAllMaths()) return;
  const active = document.activeElement;
  const focus = active && active !== document.body ? active : null;
  if (focus) {
    if (offerKey(focus)) return;
    // MathLive listens on a sink inside its shadow root, which the replayed
    // key never reaches.
    const math = focus.closest<MathfieldElement>("math-field");
    if (math) return math.select();
    if (focus.closest(EDITABLE)) {
      document.execCommand("selectAll");
      return;
    }
    const overlay = focus.closest(OVERLAY);
    if (overlay) return selectContents(overlay);
  }
  const target = currentFindTarget(pane);
  if (!target) return;
  if (target.selectAll) target.selectAll();
  else selectContents(target.root);
}

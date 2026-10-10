import { redo, undo } from "@codemirror/commands";
import type { Extension } from "@codemirror/state";
import { EditorView, ViewPlugin } from "@codemirror/view";

import { EDITABLE, onEdit } from "@/lib/menu/editRouting";

import { activeMathField } from "../math/field/mathField";

/**
 * Undo and redo reach the note's history from anywhere, not only from a
 * focused content line. On macOS the Edit menu owns ⌘Z (`shell/menu.rs`) and the
 * page never sees the key, so the menu's event (`lib/menu/editRouting.ts`) lands
 * here: inside a note editor, or with no text control focused, it steps the
 * note last used. A title field, the chat composer or any other input keeps
 * its native undo. The document-level key and `beforeinput` listeners cover
 * the keys and events that do reach the page (other platforms, a widget that
 * CodeMirror's own handlers ignore).
 */

let last: EditorView | null = null;

/** Remembers the view that last had focus. */
export function noteUndoRouting(): Extension {
  return ViewPlugin.fromClass(
    class {
      constructor(readonly view: EditorView) {
        if (view.hasFocus) last = view;
      }
      update(u: { view: EditorView; focusChanged: boolean }) {
        if (u.focusChanged && u.view.hasFocus) last = u.view;
      }
      destroy() {
        if (last === this.view) last = null;
      }
    },
  );
}

/** Hidden tabs keep their editors mounted with `visibility: hidden`. */
function showing(view: EditorView): boolean {
  return view.dom.isConnected && getComputedStyle(view.dom).visibility !== "hidden" && view.dom.getClientRects().length > 0;
}

/** The note an undo aimed at `target` belongs to, or null to leave it alone. */
function viewFor(target: EventTarget | null): EditorView | null {
  const el = target instanceof Element ? target : null;
  const dom = el?.closest(".cm-editor");
  if (dom instanceof HTMLElement) return EditorView.findFromDOM(dom);
  // Another text control (the title, a composer) undoes itself.
  if (el?.closest(EDITABLE)) return null;
  const active = document.activeElement;
  if (active && active !== document.body && active.closest(EDITABLE)) return null;
  const view = last;
  return view && showing(view) ? view : null;
}

function step(view: EditorView, redoing: boolean) {
  // Keystrokes still in a maths field go into the note first, as the field's
  // own ⌘Z does.
  activeMathField(view)?.flush();
  (redoing ? redo : undo)(view);
}

function onKeyDown(e: KeyboardEvent) {
  if (e.defaultPrevented || e.isComposing || !(e.metaKey || e.ctrlKey) || e.altKey) return;
  const key = e.key.toLowerCase();
  if (key !== "z" && !(key === "y" && !e.shiftKey)) return;
  const view = viewFor(e.target);
  if (!view) return;
  e.preventDefault();
  step(view, key === "y" || e.shiftKey);
}

function onBeforeInput(e: InputEvent) {
  if (e.defaultPrevented || (e.inputType !== "historyUndo" && e.inputType !== "historyRedo")) return;
  const view = viewFor(e.target);
  if (!view) return;
  e.preventDefault();
  step(view, e.inputType === "historyRedo");
}

if (typeof document !== "undefined") {
  const offMenu = onEdit((redoing) => {
    const view = viewFor(document.activeElement);
    if (!view) return false;
    step(view, redoing);
    return true;
  });
  document.addEventListener("keydown", onKeyDown);
  document.addEventListener("beforeinput", onBeforeInput);
  import.meta.hot?.dispose(() => {
    offMenu();
    document.removeEventListener("keydown", onKeyDown);
    document.removeEventListener("beforeinput", onBeforeInput);
  });
}

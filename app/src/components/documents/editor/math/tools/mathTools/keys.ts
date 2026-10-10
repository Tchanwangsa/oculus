import { keymap, type EditorView } from "@codemirror/view";

import { setMathMode, visualToggle, type VisualField } from "../../field/mathField";
import { quickPicks } from "../mathUsage";
import { subjectOf } from "./insert";
import { pastedField, pastedMath } from "./paste-shape";
import { pickQuick } from "./quick-picks";
import { emptyPairToBlock } from "./shape";
import { mathToolsField, mathToolsOpen, openMathTools, toggleMathTools } from "./state";

const MODIFIER_KEYS = new Set(["Shift", "Meta", "Control", "Alt", "CapsLock", "Fn"]);

/** Mod-Shift-Space, the popover's key in every mode (`toggleMathTools`). */
const toolsShortcut = (e: KeyboardEvent) => (e.metaKey || e.ctrlKey) && e.shiftKey && !e.altKey && e.code === "Space";

/**
 * The toolbox's keys in the visual field, ahead of the field's own. Space
 * opens the quick picks, then the popover; while the picks are up 1–5
 * insert one and Esc closes them, and any other key closes them and goes on
 * to the field. Esc closes the popover before it leaves the field.
 */
export function fieldKey(view: EditorView, e: KeyboardEvent, field: VisualField): boolean {
  const open = mathToolsOpen(view.state);
  if (toolsShortcut(e)) return toggleMathTools(view);
  const close = () => view.dispatch({ effects: openMathTools.of(null) });
  const plain = !e.metaKey && !e.ctrlKey && !e.altKey;
  // A `\command` being typed keeps its keys, Esc included.
  if (field.mode() === "command") {
    if (open === "quick") close();
    return false;
  }
  // `$` in an empty inline field: `$$`, a block.
  if (e.key === "$" && plain && !field.display && field.mode() === "math" && field.isEmpty()) {
    if (open) close();
    return emptyPairToBlock(view);
  }
  if (open === "quick") {
    if (MODIFIER_KEYS.has(e.key)) return false;
    if (e.key === " " && plain && !e.shiftKey) {
      view.dispatch({ effects: openMathTools.of("full") });
      return true;
    }
    const pick = plain && /^[1-9]$/.test(e.key) ? quickPicks(subjectOf(view))[Number(e.key) - 1] : undefined;
    if (pick) {
      pickQuick(view, pick);
      return true;
    }
    close();
    return e.key === "Escape";
  }
  if (open === "full" && e.key === "Escape") {
    close();
    return true;
  }
  if (!open && e.key === " " && plain && !e.shiftKey && field.spaceFree() && view.state.field(mathToolsField).math) {
    view.dispatch({ effects: openMathTools.of("quick") });
    return true;
  }
  return false;
}

/** Esc closes what is open, the paste chip last. Below the completion and snippet keymaps, whose
 *  Esc goes first. */
export const toolsKeymap = keymap.of([
  {
    key: "Escape",
    run: (view) => {
      if (mathToolsOpen(view.state)) view.dispatch({ effects: openMathTools.of(null) });
      else if (view.state.field(pastedField, false)) view.dispatch({ effects: pastedMath.of(null) });
      else return false;
      return true;
    },
  },
  { key: "Mod-Shift-Space", run: toggleMathTools },
  {
    // The field has the same key for the other way (`mathField/controller/keyboard.ts`).
    key: "Mod-Shift-m",
    run: (view) => {
      if (visualToggle(view.state) !== "visual") return false;
      setMathMode(view, "visual");
      return true;
    },
  },
]);

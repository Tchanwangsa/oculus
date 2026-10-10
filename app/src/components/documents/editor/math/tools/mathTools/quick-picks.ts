import type { EditorView, Rect, TooltipView, ViewUpdate } from "@codemirror/view";

import { activeMathField, type FieldController } from "../../field/mathField";
import type { MathEntry } from "../mathPalette";
import { quickPicks } from "../mathUsage";
import { button, cellButton, el, syncHidden } from "./dom";
import { insertEntry, subjectOf } from "./insert";
import { mathToolsOpen, openMathTools } from "./state";

const EXPAND_ICON =
  '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>';

/**
 * The visual field's quick picks: a strip hanging from the field's caret
 * like a completion list (CodeMirror flips it above near the window's
 * bottom), with the note's subject's most recent entries keyed 1–5 and a
 * button on to the popover. Any caret move in the field closes it; its keys
 * are `fieldKey`'s.
 */
export class QuickPicksView implements TooltipView {
  dom = el("div", "cm-math-tools cm-math-quick");
  overlap = true;
  offset = { x: -4, y: 4 };
  private field: FieldController | null;
  /** The field's selection the strip opened at. */
  private at: string;
  /** MathLive reports moves from inside its own updates, and late ones from
   *  before the strip opened; close after them, on a real move. */
  private moved = (e: Event) => {
    if (e.type === "selection-change" && this.selection() === this.at) return;
    queueMicrotask(() => {
      if (mathToolsOpen(this.view.state) === "quick") this.view.dispatch({ effects: openMathTools.of(null) });
    });
  };

  constructor(readonly view: EditorView) {
    this.dom.setAttribute("role", "group");
    this.dom.setAttribute("aria-label", "Quick maths picks");
    this.dom.addEventListener("mousedown", (e) => e.preventDefault());
    quickPicks(subjectOf(view)).forEach((entry, i) => {
      const cell = cellButton(entry, () => pickQuick(view, entry));
      cell.append(el("span", "cm-math-quick-key", String(i + 1)));
      this.dom.append(cell);
    });
    const expand = button("cm-math-close", "All maths tools (Space)", () =>
      view.dispatch({ effects: openMathTools.of("full") }),
    );
    expand.innerHTML = EXPAND_ICON;
    this.dom.append(expand);
    this.field = activeMathField(view);
    this.at = this.selection();
    this.field?.mf.addEventListener("selection-change", this.moved);
    this.field?.dom.addEventListener("pointerdown", this.moved);
  }

  private selection(): string {
    return JSON.stringify(this.field?.mf.selection ?? null);
  }

  mount() {
    this.update();
  }

  update(u?: ViewUpdate) {
    syncHidden(this.view, this.dom, u?.state ?? this.view.state);
  }

  /** Under the field's caret, left edges lined up; right edges instead when
   *  it would run past the edge the editor is clipped at. */
  getCoords(): Rect {
    const caret = activeMathField(this.view)?.caretRect();
    // Null hides the tooltip, as for maths scrolled out of view.
    if (!caret) return null as unknown as Rect;
    const width = this.dom.offsetWidth;
    if (caret.left + this.offset.x + width <= clipRight(this.view.dom)) return caret;
    const left = caret.left - this.offset.x * 2 - width;
    return { ...caret, left, right: left + width };
  }

  destroy() {
    this.field?.mf.removeEventListener("selection-change", this.moved);
    this.field?.dom.removeEventListener("pointerdown", this.moved);
  }
}

/** The right edge `dom` shows to: the window's, or the nearest clipping or
 *  scrolling ancestor's. */
function clipRight(dom: HTMLElement): number {
  let right = dom.ownerDocument.documentElement.clientWidth;
  for (let n: HTMLElement | null = dom; n; n = n.parentElement) {
    if (getComputedStyle(n).overflowX !== "visible") right = Math.min(right, n.getBoundingClientRect().right);
  }
  return right;
}

/** Insert a quick pick into the field, closing the strip. */
export function pickQuick(view: EditorView, entry: MathEntry) {
  view.dispatch({ effects: openMathTools.of(null) });
  insertEntry(view, entry);
}

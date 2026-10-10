import { MathField } from "@/lib/maths";
import { mathAround } from "../selection";

/** What a press on maths inside belongs to rather than the maths. */
export const NOT_MATHS_PRESS = "button, a, [role='button'], [contenteditable='true']";

/** A rendered formula drawn with the source map (`rehypeMaths`). */
export interface Formula {
  /** KaTeX's `.katex`: the box the bands are positioned in. */
  root: HTMLElement;
  /** Its `.katex-html`, the drawing `data-s`/`data-e` are on. */
  html: HTMLElement;
  /** The TeX it was rendered from, which those offsets index: the
   *  annotation's text, untrimmed. */
  tex: string;
  display: boolean;
}

/** The formula under a press's target, outside any control; null for
 *  maths drawn without the source map. */
export function formulaAt(target: EventTarget | null): Formula | null {
  const el = target instanceof Element ? target : null;
  if (!el || el.closest(NOT_MATHS_PRESS)) return null;
  const math = mathAround(el);
  if (!math) return null;
  const root = math.classList.contains("katex") ? math : math.querySelector(".katex");
  const html = root?.querySelector(":scope > .katex-html");
  const tex = root?.querySelector('annotation[encoding="application/x-tex"]')?.textContent;
  if (!(root instanceof HTMLElement) || !(html instanceof HTMLElement) || !tex || !html.querySelector("[data-s]")) {
    return null;
  }
  return { root, html, tex, display: math.classList.contains("katex-display") };
}

/** Each drawn formula's edit model, null where it would not open. */
const opened = new WeakMap<Element, { tex: string; display: boolean; field: MathField | null }>();

/** The formula's edit model, opened once per drawing; null when the model
 *  cannot read it or the engine traps on it (it then selects as text). */
export function fieldOf(formula: Formula): MathField | null {
  const { root, tex, display } = formula;
  const hit = opened.get(root);
  if (hit && hit.tex === tex && hit.display === display) return hit.field;
  hit?.field?.free();
  let field: MathField | null = null;
  try {
    field = MathField.open(tex, display);
  } catch {
    field = null;
  }
  opened.set(root, { tex, display, field });
  return field;
}

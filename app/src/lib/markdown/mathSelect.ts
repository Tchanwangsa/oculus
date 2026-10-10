import type { MathfieldElement } from "mathlive";

import { mathAround } from "./selection";

/**
 * Selecting inside a rendered formula (a chat reply, a markdown file). KaTeX's
 * output has no map from its glyphs back to the TeX, so a press on a formula
 * swaps it for a read-only MathLive field, the note editor's (`components/documents/editor/math/field/mathField/`):
 * a drag selects by structure — a cell, a matrix, a run of atoms — through
 * the editor's own hit-test (`caretAt`), widening (`wholeStructures`) and
 * copy. Focus leaving the field, Esc or a press outside it puts the KaTeX
 * back. A selection started in the prose around a formula still takes it
 * whole (`selection.ts`).
 * MathLive loads as the pointer first reaches a formula; a press before it
 * has loaded selects as plain text does.
 */

type Field = typeof import("@/components/documents/editor/math/field/mathField");

let field: Field | null = null;
/** Closes the open field, if one is. */
let openField: (() => void) | null = null;
let loading: Promise<void> | null = null;

function load() {
  loading ??= import("@/components/documents/editor/math/field/mathField").then(
    async (m) => {
      await m.loadMathLive();
      field = m;
    },
    () => {},
  );
}

/** The formula a press may open: rendered maths outside a control, whose
 *  TeX MathLive reads cleanly. */
function target(e: Event): { math: Element; tex: string; display: boolean } | null {
  const el = e.target instanceof Element ? e.target : null;
  if (!el || el.closest("button, a, [role='button'], [contenteditable='true'], math-field")) return null;
  const math = mathAround(el);
  const katex = math?.classList.contains("katex") ? math : math?.querySelector(".katex");
  const tex = katex?.querySelector('annotation[encoding="application/x-tex"]')?.textContent?.trim();
  if (!math || !katex || !tex) return null;
  return { math, tex, display: math.classList.contains("katex-display") };
}

function open(e: PointerEvent, math: Element, tex: string, display: boolean, f: Field) {
  const MF = customElements.get("math-field") as typeof MathfieldElement | undefined;
  if (!MF) return;
  const katex = math.classList.contains("katex") ? math : math.querySelector(".katex")!;
  const mf = new MF();
  if (display) f.centredRows(mf);
  mf.value = f.toField(tex, display);
  mf.className = display ? "md-math-field md-math-field-block" : "md-math-field";
  // KaTeX's own size, which the markdown around it may have scaled.
  mf.style.fontSize = getComputedStyle(katex).fontSize;
  math.setAttribute("data-math-open", "");
  math.append(mf);
  mf.readOnly = true;
  mf.defaultMode = display ? "math" : "inline-math";
  mf.mathVirtualKeyboardPolicy = "manual";
  mf.menuItems = [];
  mf.environmentPopoverPolicy = "off";
  mf.onExport = (_mf, latex) => latex;
  // Focus draws the field at once, so the press can land on its atoms.
  mf.focus();

  // A press outside closes it as well as focus leaving: MathLive takes DOM
  // focus 60 ms after `focus()`, and drops it if anything moved focus first.
  openField?.();
  const close = () => {
    if (openField === close) openField = null;
    mf.remove();
    math.removeAttribute("data-math-open");
  };
  openField = close;
  mf.addEventListener("focusout", close);
  mf.addEventListener("keydown", (k) => {
    if (k.key === "Escape") close();
  });
  // A drag into or out of a structure takes it whole, as in the editor.
  mf.addEventListener("selection-change", () => {
    const model = f.modelOf(mf);
    const { ranges } = mf.selection;
    if (!model || ranges.length !== 1 || ranges[0][0] === ranges[0][1]) return;
    const [start, end] = ranges[0][0] < ranges[0][1] ? ranges[0] : [ranges[0][1], ranges[0][0]];
    const whole = f.wholeStructures(model, start, end);
    if (whole) mf.selection = { ranges: [whole], direction: mf.position === start ? "backward" : "forward" };
  });
  // Bubbling, after MathLive has put its LaTeX on the clipboard: the TeX
  // without its `\displaylines` wrapper, and a block's `$$` lines for a
  // paste into a note (`BLOCK_MATH_TYPE`).
  mf.addEventListener("copy", (c) => {
    const data = c.clipboardData;
    const text = data?.getData("text/plain");
    if (!data || !text) return;
    const latex = f.fromField(f.tidy(text, !display));
    data.setData("text/plain", latex);
    if (display) data.setData(f.BLOCK_MATH_TYPE, `$$\n${f.layoutBlock(latex)}\n$$`);
  });

  // The press itself, handed on so this one drag already selects in the
  // field; a plain click then takes the editor's caret.
  mf.dispatchEvent(
    new PointerEvent("pointerdown", {
      bubbles: true,
      composed: true,
      cancelable: true,
      clientX: e.clientX,
      clientY: e.clientY,
      screenX: e.screenX,
      screenY: e.screenY,
      button: e.button,
      buttons: e.buttons,
      detail: e.detail,
      pointerId: e.pointerId,
      pointerType: e.pointerType,
      isPrimary: e.isPrimary,
    }),
  );
  const caret = f.caretAt(mf, e.clientX, e.clientY);
  if (caret != null) mf.position = caret;
}

/** Installed once, app-wide. */
export function watchMathPress() {
  document.addEventListener("pointerover", (e) => {
    if (!loading && target(e)) load();
  });
  document.addEventListener(
    "pointerdown",
    (e) => {
      if (openField && !(e.target instanceof Element && e.target.closest("math-field"))) openField();
      if (e.button !== 0 || e.shiftKey || e.metaKey || e.ctrlKey || !field) return;
      const t = target(e);
      if (!t || !field.readsCleanly(t.tex, t.display)) return;
      // No native text selection from this press: the field takes it.
      e.preventDefault();
      open(e, t.math, t.tex, t.display, field);
    },
    true,
  );
}

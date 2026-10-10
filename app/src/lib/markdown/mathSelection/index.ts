import { copied } from "./clipboard";
import { fieldOf, formulaAt } from "./formula";
import { Session } from "./session";

/**
 * Selecting inside a rendered formula (a chat reply, a markdown file) in
 * place. `rehypeMaths` draws the maths with the source map,
 * and a press runs the note field's edit model (`MathField`) read-only over
 * that drawing: the geometry (`lib/maths/geometry`) finds the stop under
 * the pointer, a drag selects from the press to it, the model widening the
 * selection over whole structures, and one band per row is drawn under the
 * glyphs. Nothing is swapped in and no glyph moves.
 *
 * A press with no drag selects nothing (no caret is drawn) and closes on
 * release; Shift extends from the last press. A press elsewhere, Esc,
 * focus moving or a text selection starting closes it. Copy writes the
 * selected TeX (`clipboard.ts`), Select All takes the model's select-all.
 * Maths the model cannot open selects as text does, whole
 * (`selection.ts`).
 */

let active: Session | null = null;
/** A press this module took: its `mousedown` is cancelled, so no text
 *  selection starts and focus stays put (cancelling the `pointerdown`
 *  would kill WebKit's click, docs/ui.md). */
let taken = false;

function close() {
  active?.close();
  active = null;
}

/** The press's focus loss, as a press elsewhere would: typing no longer
 *  goes to the composer, and focus coming back closes the selection. */
function blurFocus() {
  const focused = document.activeElement;
  if (focused instanceof HTMLElement && focused !== document.body) focused.blur();
}

function pointerDown(e: PointerEvent) {
  const plain = e.button === 0 && !e.metaKey && !e.ctrlKey && !e.altKey;
  const formula = plain ? formulaAt(e.target) : null;
  const base = formula ? fieldOf(formula) : null;
  if (active && (!formula || active.formula.root !== formula.root || active.base !== base || !active.connected)) close();
  if (!formula || !base || (e.shiftKey && !active)) return;

  const session = active ?? new Session(formula, base);
  const at = session.stopAt(e.clientX, e.clientY);
  if (at == null) {
    if (session !== active) session.close();
    return;
  }
  active = session;
  taken = true;
  window.getSelection()?.removeAllRanges();
  blurFocus();
  if (!e.shiftKey) session.anchor = at;
  const anchor = session.anchor;
  if (!session.select(anchor, at)) return close();

  let head = at;
  const move = (m: PointerEvent) => {
    m.preventDefault();
    if (active !== session) return;
    const next = session.stopAt(m.clientX, m.clientY);
    if (next == null || next === head) return;
    head = next;
    if (!session.select(anchor, head)) close();
  };
  const end = () => {
    taken = false;
    window.removeEventListener("pointermove", move, true);
    window.removeEventListener("pointerup", end, true);
    window.removeEventListener("pointercancel", end, true);
    if (active === session && session.empty) close();
  };
  window.addEventListener("pointermove", move, true);
  window.addEventListener("pointerup", end, true);
  window.addEventListener("pointercancel", end, true);
}

/** Whether a maths selection owns the clipboard: one is shown and no text
 *  selection competes with it. */
function owned(): Session | null {
  if (!active || active.empty || !active.connected) return null;
  const selection = window.getSelection();
  return selection && selection.rangeCount && !selection.isCollapsed ? null : active;
}

/** Select All with a maths selection shown: the model's select-all in that
 *  formula (`lib/menu/editRouting.ts`); false when none is shown. */
export function selectAllMaths(): boolean {
  if (!active || !active.connected) return false;
  if (!active.selectAll()) close();
  return true;
}

/** Installed once, app-wide. */
export function watchMathSelect() {
  document.addEventListener("pointerdown", pointerDown, true);
  document.addEventListener(
    "mousedown",
    (e) => {
      if (taken) e.preventDefault();
    },
    true,
  );
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && active) close();
  });
  document.addEventListener("focusin", () => close());
  document.addEventListener("selectionchange", () => {
    const selection = window.getSelection();
    if (active && selection && selection.rangeCount && !selection.isCollapsed) close();
  });
  // WebKit enables Copy only for a text selection; cancelling `beforecopy`
  // says the page copies.
  document.addEventListener(
    "beforecopy",
    (e) => {
      if (owned()) e.preventDefault();
    },
    true,
  );
  document.addEventListener(
    "copy",
    (e) => {
      const session = owned();
      if (!session || !e.clipboardData) return;
      const entries = copied(session.field.source, session.field.selected, session.formula.display);
      if (!entries.length) return;
      for (const [type, data] of entries) e.clipboardData.setData(type, data);
      e.preventDefault();
      e.stopPropagation();
    },
    true,
  );
}

import { extendUnit, type StopRange } from "@/lib/maths/geometry/units";
import { selectParagraph } from "./paragraph";
import { copied } from "./clipboard";
import { fieldOf, formulaAt } from "./formula";
import { Session } from "./session";

export { watchMathSelection } from "./whole";

/**
 * Read-only selection in rendered chat and file maths. The source map and
 * field model choose caret stops for a drag, atoms for a double-click and
 * enclosing structures for a triple-click. Bands draw under the glyphs.
 * A third prose click takes the whole paragraph, including inline maths.
 *
 * An empty press closes on release; Shift extends from the last press.
 * A press elsewhere, Esc, focus moving or native selection closes maths.
 * Copy wraps the selected TeX by its shape; Select All takes the formula.
 * Maths the model cannot open stays with native selection and copies whole.
 */

let active: Session | null = null;
/** A press this module took: its `mousedown` is cancelled, so no text
 *  selection starts and focus stays put (cancelling the `pointerdown`
 *  would kill WebKit's click, docs/ui.md). */
let taken = false;
let press: { session: Session; unit: StopRange | null; group: boolean } | null = null;

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
  const gesture = { session, unit: null as StopRange | null, group: false };
  press = gesture;
  taken = true;
  window.getSelection()?.removeAllRanges();
  blurFocus();
  if (!e.shiftKey) session.anchor = at;
  const anchor = session.anchor;
  if (!session.select(anchor, at)) return close();

  let head = at;
  const move = (m: PointerEvent) => {
    m.preventDefault();
    if (active !== session || m.pointerId !== e.pointerId) return;
    if (gesture.unit) {
      const next = session.unitAt(m.clientX, m.clientY, gesture.group);
      if (next) {
        const range = extendUnit(gesture.unit, next);
        session.anchor = range[0];
        if (!session.select(...range)) close();
      }
      return;
    }
    const next = session.stopAt(m.clientX, m.clientY);
    if (next == null || next === head) return;
    head = next;
    if (!session.select(anchor, head)) close();
  };
  const end = () => {
    if (press === gesture) press = null;
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
      if (taken) {
        e.preventDefault();
        if (!press || e.detail < 2 || e.shiftKey) return;
        press.group = e.detail >= 3;
        press.unit = press.session.unitAt(e.clientX, e.clientY, press.group);
        if (press.unit) {
          press.session.anchor = press.unit[0];
          if (!press.session.select(...press.unit)) close();
        }
      } else if (e.button === 0 && e.detail === 3 && !e.shiftKey && !e.metaKey && !e.ctrlKey && !e.altKey && selectParagraph(e.target)) {
        e.preventDefault();
      }
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
      const entries = copied(session.field, session.formula.display);
      if (!entries.length) return;
      for (const [type, data] of entries) e.clipboardData.setData(type, data);
      e.preventDefault();
      e.stopPropagation();
    },
    true,
  );
}

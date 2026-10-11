import { EditorView } from "@codemirror/view";

import type { Box } from "@/lib/maths/geometry";

/**
 * How the field scrolls the note. The caret it draws, or the note's caret
 * beside the maths after leaving, is kept on screen: the note's scroller
 * moves only when the caret is out of its visible band (less the sticky
 * toolbar's `scrollMargins`), and then just enough to bring it to the
 * nearer edge. CodeMirror's own `scrollIntoView` measures a position inside
 * the field as the whole widget. Focus going back to the note never
 * scrolls (`focusNote`).
 */

/** Px kept between the caret and the visible band's edge, as CodeMirror's
 *  `scrollIntoView` keeps by default. */
const MARGIN = 5;

/** How far to scroll (positive is down) so `[top, bottom]` sits inside
 *  `[viewTop, viewBottom]`, the nearer edge first; 0 when it already does.
 *  A caret taller than the band keeps its top in view. */
export function nearestScroll(top: number, bottom: number, viewTop: number, viewBottom: number): number {
  if (top < viewTop) return top - viewTop;
  if (bottom > viewBottom) return Math.min(bottom - viewBottom, top - viewTop);
  return 0;
}

const scrolls = (el: Element) => {
  const y = getComputedStyle(el).overflowY;
  return el.scrollHeight > el.clientHeight && y !== "visible" && y !== "clip";
};

/** The note's vertical scroller: the first box above the editor that can
 *  scroll and overflows (as patched CodeMirror finds it), else the page. */
function scroller(view: EditorView): Element | null {
  for (let el: Element | null = view.scrollDOM; el; el = el.parentElement) if (scrolls(el)) return el;
  return document.scrollingElement;
}

/**
 * `view.focus()` with every scroller above the editor held where it was.
 * WebKit 26 scrolls to the note's old DOM selection on focus even with
 * `preventScroll`, which CodeMirror's feature test reads as supported: a
 * line just off screen came back centred, a jump of hundreds of px.
 */
export function focusNote(view: EditorView) {
  const held: [Element, number, number][] = [];
  for (let el: Element | null = view.scrollDOM; el; el = el.parentElement) {
    if (el.scrollHeight > el.clientHeight || el.scrollWidth > el.clientWidth) held.push([el, el.scrollTop, el.scrollLeft]);
  }
  view.focus();
  for (const [el, top, left] of held) {
    if (el.scrollTop !== top) el.scrollTop = top;
    if (el.scrollLeft !== left) el.scrollLeft = left;
  }
}

/** The visible band of `el` in viewport px, less the editor's scroll
 *  margins and `MARGIN`. */
function visibleBand(view: EditorView, el: Element): { top: number; bottom: number } {
  let top = 0;
  let bottom = 0;
  for (const source of view.state.facet(EditorView.scrollMargins)) {
    const m = source(view);
    if (m?.top != null) top = Math.max(top, m.top);
    if (m?.bottom != null) bottom = Math.max(bottom, m.bottom);
  }
  const box = el === document.scrollingElement ? { top: 0, bottom: window.innerHeight } : el.getBoundingClientRect();
  return {
    top: Math.max(box.top, 0) + top + MARGIN,
    bottom: Math.min(box.bottom, window.innerHeight) - bottom - MARGIN,
  };
}

/** Scrolls the note, once CodeMirror has measured, so the rect `caret()`
 *  reads then is visible; nothing moves when it already is. */
export function keepVisible(view: EditorView, caret: () => Box | null) {
  view.requestMeasure({
    key: keepVisible,
    read: () => {
      const rect = caret();
      const el = rect && scroller(view);
      if (!rect || !el) return null;
      const band = visibleBand(view, el);
      const by = nearestScroll(rect.top, rect.bottom, band.top, band.bottom);
      return by ? { el, by } : null;
    },
    write: (move) => {
      if (move) move.el.scrollTop += move.by;
    },
  });
}

/** `keepVisible` for the note's caret at `pos`. */
export function keepPosVisible(view: EditorView, pos: number) {
  keepVisible(view, () => view.coordsAtPos(pos, 1) ?? view.coordsAtPos(pos, -1));
}

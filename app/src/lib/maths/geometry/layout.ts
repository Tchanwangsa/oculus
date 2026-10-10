import { area, union, type Box } from "./box";

/** One source-mapped element of a rendering (`sourceMap: true`). */
export interface MappedBox {
  /** Its UTF-16 range of the source (`data-s`/`data-e`). */
  from: number;
  to: number;
  /** Its own border box: an inline span's is its font's line, the height a
   *  caret beside it takes. */
  box: Box;
  /** Its box with everything drawn inside it: a fraction's both parts, a
   *  matrix's delimiters. */
  ink: Box;
  /** An empty slot's marker (`oc-placeholder`, `oc-empty-row`), with a
   *  zero-width range. */
  placeholder: boolean;
}

/** A rendering's mapped elements in document order, in `frame`'s
 *  coordinates. Read once per render; the functions over it are pure. */
export interface Layout {
  readonly items: readonly MappedBox[];
}

/** Elements that draw nothing a selection or a click should reach: KaTeX's
 *  vertical struts (a `pstrut` reaches far above its row), the MathML copy,
 *  a top-level line break (a full-width block), and SVG, whose `\sqrt`
 *  tail runs 400em under a clip that only its parent's box shows. */
const UNDRAWN = new Set(["pstrut", "katex-strut", "vlist-s", "katex-newline", "katex-mathml"]);

function undrawn(el: Element): boolean {
  if (el.namespaceURI === "http://www.w3.org/2000/svg") return true;
  for (const c of el.classList) if (UNDRAWN.has(c)) return true;
  return false;
}

/** The viewport position of `frame`'s (0, 0): its padding box's corner, moved
 *  by its scroll, where an absolutely positioned child at (0, 0) sits. */
export function frameOrigin(frame: Element): { x: number; y: number } {
  const r = frame.getBoundingClientRect();
  return { x: r.left + frame.clientLeft - frame.scrollLeft, y: r.top + frame.clientTop - frame.scrollTop };
}

/** A viewport point (a pointer event's) in `frame`'s coordinates. */
export function framePoint(frame: Element, clientX: number, clientY: number): { x: number; y: number } {
  const o = frameOrigin(frame);
  return { x: clientX - o.x, y: clientY - o.y };
}

/**
 * Reads the mapped elements under `root` (rendered maths), each box in
 * `frame`'s coordinates (`root` by default), so overlays positioned in
 * `frame` line up with them. Page zoom needs no scaling: rects and
 * positioned overlays are both CSS px of the zoomed layout.
 */
export function readLayout(root: Element, frame: Element = root): Layout {
  const o = frameOrigin(frame);
  const items: MappedBox[] = [];
  const walk = (el: Element): Box | null => {
    if (undrawn(el)) return null;
    const r = el.getBoundingClientRect();
    const own: Box = { left: r.left - o.x, top: r.top - o.y, right: r.right - o.x, bottom: r.bottom - o.y };
    let ink: Box | null = area(own) > 0 ? own : null;
    const s = el.getAttribute("data-s");
    const at = s == null ? -1 : items.length;
    if (s != null) {
      const from = Number(s);
      const to = Number(el.getAttribute("data-e") ?? s);
      const placeholder = el.classList.contains("oc-placeholder") || el.classList.contains("oc-empty-row");
      items.push({ from, to, box: own, ink: own, placeholder });
    }
    for (const child of el.children) {
      const c = walk(child);
      if (c) ink = ink ? union(ink, c) : c;
    }
    if (at >= 0 && ink) items[at].ink = ink;
    return ink;
  };
  walk(root);
  return { items };
}

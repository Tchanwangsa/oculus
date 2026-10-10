import type { Box } from "./box";
import type { Layout, MappedBox } from "./layout";
import { inSlot, type FieldShape } from "./shape";

/** Where each stop's caret is drawn, by stop id, in the layout's frame; NaN
 *  where nothing maps (the edit model's ↑/↓ then fall back on their own). */
export interface Measured {
  x: Float64Array;
  top: Float64Array;
  bottom: Float64Array;
}

const blank = (s: string) => s.trim() === "";

/** Ems of the caret above and below its baseline: a parenthesis' height
 *  and depth, so it is one em of the font it stands in at any size. */
const CARET_ASCENT = 0.75;
const CARET_DESCENT = 0.25;

/** The caret's top and bottom beside an element, from its baseline and font
 *  size, never its box, whose height is its font face's. */
const line = (item: MappedBox): [number, number] => [
  item.baseline - CARET_ASCENT * item.size,
  item.baseline + CARET_DESCENT * item.size,
];

/** Its own box for x, or its ink when its own box has no width. */
const edges = (item: MappedBox): Box => (item.box.right > item.box.left ? item.box : item.ink);

/**
 * Each stop's caret. A stop at offset `o` in slot `S` sits at the right
 * edge of the atom ending at `o` (the widest, so `x^2` beats its `2`), else
 * the left edge of the atom starting there, else at the empty slot's
 * marker; only source whitespace may lie between. Only elements inside
 * `S`'s interior count.
 */
export function measure(layout: Layout, field: FieldShape): Measured {
  const n = field.stops.length;
  const out: Measured = { x: new Float64Array(n), top: new Float64Array(n), bottom: new Float64Array(n) };
  const { source } = field;
  for (let id = 0; id < n; id++) {
    const o = field.stops[id];
    const slot = field.slots[field.stopSlots[id]];
    let before: MappedBox | null = null;
    let after: MappedBox | null = null;
    let marker: MappedBox | null = null;
    for (const item of layout.items) {
      if (item.placeholder) {
        if (item.from === o && item.from >= slot.from && item.from <= slot.to) marker ??= item;
        continue;
      }
      if (item.to <= item.from || !inSlot(item, slot)) continue;
      if (item.to <= o && blank(source.slice(item.to, o))) {
        if (!before || item.to > before.to || (item.to === before.to && item.from < before.from)) before = item;
      } else if (item.from >= o && blank(source.slice(o, item.from))) {
        if (!after || item.from < after.from || (item.from === after.from && item.to > after.to)) after = item;
      }
    }
    const at: [number, MappedBox | null] =
      before ? [edges(before).right, before]
      : after ? [edges(after).left, after]
      : marker ? [marker.box.left, marker]
      : fallback(layout, slot);
    const [top, bottom] = at[1] ? line(at[1]) : [NaN, NaN];
    out.x[id] = at[0];
    out.top[id] = top;
    out.bottom[id] = bottom;
  }
  return out;
}

/** An empty slot the renderer draws no marker for (such as `\mathop{}`'s
 *  argument, an `aligned` cell after a `&`, an empty row): the middle of the smallest
 *  element around it, on that element's baseline and at its size. */
function fallback(layout: Layout, slot: { from: number; to: number }): [number, MappedBox | null] {
  let around: MappedBox | null = null;
  for (const item of layout.items) {
    if (item.placeholder || item.from > slot.from || item.to < slot.to) continue;
    if (!around || item.to - item.from < around.to - around.from) around = item;
  }
  if (!around) return [NaN, null];
  return [(around.ink.left + around.ink.right) / 2, around];
}

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

/** The caret's height beside an element: its own line (a script's is
 *  smaller), or its ink when its own box has none. */
const height = (item: MappedBox): Box => (item.box.bottom > item.box.top ? item.box : item.ink);

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
    const [x, line] =
      before ? [edges(before).right, height(before)]
      : after ? [edges(after).left, height(after)]
      : marker ? [marker.box.left, height(marker)]
      : fallback(layout, slot);
    out.x[id] = x;
    out.top[id] = line ? line.top : NaN;
    out.bottom[id] = line ? line.bottom : NaN;
  }
  return out;
}

/** An empty slot the renderer draws no marker for (such as `\mathop{}`'s
 *  argument, an `aligned` cell after a `&`, an empty row): the middle of
 *  the smallest element around it. */
function fallback(layout: Layout, slot: { from: number; to: number }): [number, Box | null] {
  let around: MappedBox | null = null;
  for (const item of layout.items) {
    if (item.placeholder || item.from > slot.from || item.to < slot.to) continue;
    if (!around || item.to - item.from < around.to - around.from) around = item;
  }
  if (!around) return [NaN, null];
  return [(around.ink.left + around.ink.right) / 2, height(around)];
}

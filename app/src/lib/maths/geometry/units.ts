import { area, type Box } from "./box";
import type { Layout } from "./layout";
import type { FieldShape } from "./shape";

export type StopRange = readonly [number, number];

/** Stops in the innermost slot covering a mapped atom or structure. Stops
 *  keep commands and braces whole even when the rendering maps just ink. */
function covering(field: FieldShape, from: number, to: number): StopRange | null {
  let picked: StopRange | null = null;
  let width = Infinity;
  field.slots.forEach((slot, s) => {
    if (slot.from > from || slot.to < to) return;
    let before = -1;
    let after = -1;
    for (let id = 0; id < field.stops.length; id++) {
      if (field.stopSlots[id] !== s) continue;
      if (field.stops[id] <= from) before = id;
      if (after < 0 && field.stops[id] >= to) after = id;
    }
    if (before < 0 || after <= before || slot.to - slot.from >= width) return;
    picked = [before, after];
    width = slot.to - slot.from;
  });
  return picked;
}

const distance = (box: Box, x: number, y: number) =>
  Math.max(box.left - x, 0, x - box.right) ** 2 + Math.max(box.top - y, 0, y - box.bottom) ** 2;

/** Double-click takes the smallest ink under the pointer. Triple-click
 *  takes its nearest enclosing structure, or its row when it has none. */
export function unitAt(layout: Layout, field: FieldShape, x: number, y: number, group = false): StopRange | null {
  let picked: StopRange | null = null;
  let gap = Infinity;
  let size = Infinity;
  let span = Infinity;
  for (const item of layout.items) {
    if (item.placeholder || item.to <= item.from || area(item.ink) <= 0) continue;
    const range = covering(field, item.from, item.to);
    if (!range) continue;
    const d = distance(item.ink, x, y);
    const a = area(item.ink);
    const length = item.to - item.from;
    if (d < gap || (d === gap && (a < size || (a === size && length < span)))) {
      [picked, gap, size, span] = [range, d, a, length];
    }
  }
  if (!picked || !group) return picked;
  const [from, to] = picked.map((id) => field.stops[id]);
  const structures = layout.items.filter((item) =>
    item.from <= from && item.to >= to && field.slots.some((slot) =>
      slot.parent != null && slot.from >= item.from && slot.to <= item.to &&
      (slot.from > item.from || slot.to < item.to)),
  ).sort((a, b) => (a.to - a.from) - (b.to - b.from));
  for (const item of structures) {
    const range = covering(field, item.from, item.to);
    if (range) return range;
  }
  const slot = field.slots[field.stopSlots[picked[0]]];
  return covering(field, slot.from, slot.to) ?? picked;
}

/** Dragging after a multi-click extends by whole units in either direction. */
export function extendUnit(origin: StopRange, next: StopRange): StopRange {
  if (next[1] <= origin[0]) return [origin[1], next[0]];
  if (next[0] >= origin[1]) return [origin[0], next[1]];
  return [Math.min(origin[0], next[0]), Math.max(origin[1], next[1])];
}

import { area, holds, union, type Box } from "./box";
import type { Layout } from "./layout";
import type { Measured } from "./measure";
import { inSlot, type FieldShape } from "./shape";

/** How far past its ink a slot (a script, a numerator) still takes a press. */
const SLOT_REACH = 4;

/** Each slot's box: its atoms' ink and its stops' carets; null for a slot
 *  with neither. */
function slotBoxes(layout: Layout, field: FieldShape, measured: Measured): (Box | null)[] {
  const boxes: (Box | null)[] = field.slots.map((slot) => {
    let box: Box | null = null;
    for (const item of layout.items) {
      const inside = item.placeholder ? item.from >= slot.from && item.from <= slot.to : inSlot(item, slot);
      if (inside) box = box ? union(box, item.ink) : item.ink;
    }
    return box;
  });
  for (let id = 0; id < field.stops.length; id++) {
    const x = measured.x[id];
    if (!Number.isFinite(x)) continue;
    const caret: Box = { left: x, right: x, top: measured.top[id], bottom: measured.bottom[id] };
    const s = field.stopSlots[id];
    boxes[s] = boxes[s] ? union(boxes[s], caret) : caret;
  }
  return boxes;
}

/** The slot's top-level row: the root it hangs from. */
function rootOf(field: FieldShape, slot: number): number {
  for (let p = field.slots[slot].parent; p != null; p = field.slots[p].parent) slot = p;
  return slot;
}

/**
 * The stop a press at (x, y), in the layout's frame, puts the caret at; null
 * when no stop is measured. A display's rows: the one whose band is nearest
 * in y. In it, the smallest slot whose box (a little widened) holds the
 * point, else the row itself, which takes any x; then that slot's stop
 * nearest in x.
 */
export function stopAt(layout: Layout, field: FieldShape, measured: Measured, x: number, y: number): number | null {
  const boxes = slotBoxes(layout, field, measured);
  let row = -1;
  let gap = Infinity;
  field.slots.forEach((slot, i) => {
    const box = boxes[i];
    if (slot.parent != null || !box) return;
    const d = y < box.top ? box.top - y : y > box.bottom ? y - box.bottom : 0;
    if (d < gap) [row, gap] = [i, d];
  });
  if (row < 0) return null;
  let best = row;
  let size = Infinity;
  field.slots.forEach((slot, i) => {
    const box = boxes[i];
    if (slot.parent == null || !box || rootOf(field, i) !== row || !holds(box, x, y, SLOT_REACH)) return;
    const a = area(box);
    if (a < size) [best, size] = [i, a];
  });
  let pick: number | null = null;
  for (let id = 0; id < field.stops.length; id++) {
    const sx = measured.x[id];
    if (field.stopSlots[id] !== best || !Number.isFinite(sx)) continue;
    if (pick == null || Math.abs(sx - x) < Math.abs(measured.x[pick] - x)) pick = id;
  }
  return pick;
}

import type { FieldSlot } from "../../../../math-core/pkg/oculus_math.js";
import type { MappedBox } from "./layout";

/** What geometry reads of a `MathField`: its source, stops and slots. */
export interface FieldShape {
  readonly source: string;
  readonly stops: ArrayLike<number>;
  readonly stopSlots: ArrayLike<number>;
  readonly slots: readonly Pick<FieldSlot, "from" | "to" | "parent">[];
}

/** Whether the item lies inside the slot's interior. A parent atom's range
 *  reaches past the slot, so its box never stands in for the slot's own. */
export const inSlot = (item: MappedBox, slot: { from: number; to: number }): boolean =>
  item.from >= slot.from && item.to <= slot.to;

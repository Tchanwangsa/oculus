import { bands, paintBands } from "@/lib/maths/geometry";
import type { MathView } from "./index";

/** The selection's bands, one per row (`bands`). */
export function drawBands(view: MathView) {
  const [from, to] = view.field.selected;
  const rows = view.field.slots.filter((s) => s.parent == null);
  paintBands(view.bandLayer, bands(view.layout, from, to, rows), "cm-math-view-band");
}

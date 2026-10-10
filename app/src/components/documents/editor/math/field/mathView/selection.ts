import { bands, paintBands } from "@/lib/maths/geometry";
import type { MathView } from "./index";

/** The selection's bands over the selected atoms (`bands`). */
export function drawBands(view: MathView) {
  const [from, to] = view.field.selected;
  paintBands(view.bandLayer, bands(view.layout, from, to, view.field.slots), "cm-math-view-band");
}

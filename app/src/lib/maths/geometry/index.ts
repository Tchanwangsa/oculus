/**
 * Geometry of source-mapped maths (`renderToString` with `sourceMap: true`)
 * for the visual field and read-only selection: where each caret stop is
 * drawn, which stop a press lands on, and a selection's highlight bands.
 * `readLayout` reads the DOM once per render and `paintBands` draws the
 * bands; the rest is pure over the layout.
 */
export { area, holds, union, type Box } from "./box";
export { bands } from "./bands";
export { stopAt } from "./hit";
export { framePoint, frameOrigin, readLayout, type Layout, type MappedBox } from "./layout";
export { measure, type Measured } from "./measure";
export { paintBands } from "./paint";
export type { FieldShape } from "./shape";

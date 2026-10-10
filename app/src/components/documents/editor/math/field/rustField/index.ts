/**
 * The Rust visual field in the note: `MathView` (`math/field/mathView`)
 * hosted on the maths it edits, behind the same `VisualField` contract as
 * MathLive's field. `fieldEngine.ts`'s switch picks it as a field opens.
 */
export { RustFieldController, openRustField } from "./controller";

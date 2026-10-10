/** The `localStorage` key that picks the visual field's engine. */
export const MATH_ENGINE_KEY = "oculus-math-engine";

/**
 * Whether maths is edited and selected through the Rust edit model rather
 * than MathLive: `localStorage["oculus-math-engine"]` is `"rust"`. It picks
 * the note's visual field (`components/documents/editor/math/field/`) and
 * how rendered markdown's maths is selected (`lib/markdown/mathSelection/`).
 * Read each time it matters, so switching needs no reload; storage that
 * can't be read keeps MathLive.
 */
export function rustField(): boolean {
  try {
    return globalThis.localStorage?.getItem(MATH_ENGINE_KEY) === "rust";
  } catch {
    return false;
  }
}

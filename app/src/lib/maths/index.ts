/** The maths engine (`math-core/wasm`, a Rust port of KaTeX 0.18.5) behind
 *  KaTeX's API: load it once, then render synchronously. */
export { MathsTrap, mathsReady, onMathsReady } from "./engine";
export { loadMaths } from "./load";
export { parseError, renderToString, type MathOptions } from "./render";
export { default as rehypeMaths } from "./rehypeMaths";
export { useMathsReady } from "./useMathsReady";

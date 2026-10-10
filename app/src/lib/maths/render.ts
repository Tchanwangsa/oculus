import type { MathOptions } from "../../../math-core/pkg/oculus_math.js";
import { callMaths } from "./engine";

export type { MathOptions };

/**
 * KaTeX's `renderToString`, drawn by the engine: KaTeX 0.18.5's output, its
 * option names and defaults (`throwOnError` true, `strict` "warn"). A parse
 * error throws an `Error` named `ParseError` with KaTeX's message
 * (`KaTeX parse error: …`); with `throwOnError: false` it returns KaTeX's red
 * `span.katex-error` instead. A trap throws a `MathsTrap` whatever the
 * options, as KaTeX rethrows errors that aren't its own. Only once
 * `mathsReady()`.
 */
export function renderToString(tex: string, options?: MathOptions): string {
  return callMaths(tex, (glue) => glue.renderToString(tex, options));
}

/** The message `renderToString` would throw with `throwOnError` on, or
 *  undefined when `tex` renders; a trap is an error too. Only once
 *  `mathsReady()`. */
export function parseError(tex: string, options?: MathOptions): string | undefined {
  try {
    return callMaths(tex, (glue) => glue.parseError(tex, options));
  } catch (e) {
    if (e instanceof Error && e.name === "MathsTrap") return e.message;
    throw e;
  }
}

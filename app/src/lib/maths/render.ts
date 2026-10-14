import type { MathOptions } from "../../../math-core/pkg/oculus_math.js";
import { callMaths } from "./engine";

export type { MathOptions };

/** A render, or the `ParseError` it threw. */
type Cached = { html: string; error?: undefined } | { html?: undefined; error: Error };

// The editor's field and tools re-render the same formulas on every keystroke
// and redraw, so renders are kept by source and options, least recently used
// dropped first, up to MAX_ENTRIES and MAX_CHARS (keys and HTML, in UTF-16 units).
const MAX_ENTRIES = 2000;
const MAX_CHARS = 4_000_000;
const cache = new Map<string, Cached>();
let cachedChars = 0;

const own = (o: object, k: string): unknown =>
  Object.prototype.hasOwnProperty.call(o, k) ? (o as Record<string, unknown>)[k] : undefined;

/** The flag for a boolean option: `d` for its default, or null when the
 *  value is malformed (the engine throws a `TypeError`, which isn't cached). */
function flag(value: unknown, d: boolean): string | null {
  if (value == null) return d ? "1" : "0";
  return typeof value === "boolean" ? (value ? "1" : "0") : null;
}

const STRICT = new Map<unknown, string>([[true, "T"], [false, "F"], ["ignore", "i"], ["warn", "w"], ["error", "e"]]);
const OUTPUT = new Map<unknown, string>([["htmlAndMathml", "b"], ["html", "h"], ["mathml", "m"]]);

/** Every option that changes the output, defaults made explicit, then `tex`;
 *  null when an option is malformed. The macros are a JSON array of entries
 *  sorted by name, which can't run into the TeX after it. Exported for tests. */
export function renderCacheKey(tex: string, options: MathOptions | undefined): string | null {
  if (options == null) return `010wb[]${tex}`;
  if (typeof options !== "object") return null;
  const display = flag(own(options, "displayMode"), false);
  const throwOnError = flag(own(options, "throwOnError"), true);
  const sourceMap = flag(own(options, "sourceMap"), false);
  const strictValue = own(options, "strict");
  const strict = strictValue == null ? "w" : STRICT.get(strictValue);
  const outputValue = own(options, "output");
  const output = outputValue == null ? "b" : OUTPUT.get(outputValue);
  if (display == null || throwOnError == null || sourceMap == null || !strict || !output) return null;
  const macros = own(options, "macros");
  let macroKey = "[]";
  if (macros != null) {
    if (typeof macros !== "object" || Array.isArray(macros)) return null;
    const entries = Object.entries(macros);
    if (entries.some(([, body]) => typeof body !== "string")) return null;
    if (entries.length) macroKey = JSON.stringify(entries.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
  }
  return `${display}${throwOnError}${sourceMap}${strict}${output}${macroKey}${tex}`;
}

function remember(key: string, entry: Cached) {
  const size = key.length + (entry.html ?? entry.error!.message).length;
  if (size > MAX_CHARS) return;
  cache.set(key, entry);
  cachedChars += size;
  while (cache.size > MAX_ENTRIES || cachedChars > MAX_CHARS) {
    const [oldKey, old] = cache.entries().next().value!;
    cache.delete(oldKey);
    cachedChars -= oldKey.length + (old.html ?? old.error!.message).length;
  }
}

/**
 * KaTeX's `renderToString`, drawn by the engine: KaTeX 0.18.5's output, its
 * option names and defaults (`throwOnError` true, `strict` "warn"). A parse
 * error throws an `Error` named `ParseError` with KaTeX's message
 * (`KaTeX parse error: …`); with `throwOnError: false` it returns KaTeX's red
 * `span.katex-error` instead. A trap throws a `MathsTrap` whatever the
 * options, as KaTeX rethrows errors that aren't its own. Only once
 * `mathsReady()`. A render is a pure function of `tex` and the options (the
 * engine copies `macros`, so a `\gdef` never writes back), so renders and
 * parse errors are cached; a hit rethrows the same `ParseError`.
 */
export function renderToString(tex: string, options?: MathOptions): string {
  const key = renderCacheKey(tex, options);
  if (key == null) return callMaths(tex, (glue) => glue.renderToString(tex, options));
  const hit = cache.get(key);
  if (hit) {
    cache.delete(key);
    cache.set(key, hit);
    if (hit.error) throw hit.error;
    return hit.html;
  }
  try {
    const html = callMaths(tex, (glue) => glue.renderToString(tex, options));
    remember(key, { html });
    return html;
  } catch (e) {
    if (e instanceof Error && e.name === "ParseError") remember(key, { error: e });
    throw e;
  }
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

import type { InlineShortcutDefinitions } from "mathlive";

import { GREEK, OPERATORS, POWERS } from "../../tools/shorthand/tables";

/** MathLive defaults that turn ordinary letter runs (variable names, prose)
 *  into units, words or rare function aliases. */
const PRUNED = [
  "in", "!in", "of", "and", "or", "not", "sub", "sup", "sube", "supe", "mod", "(mod",
  "mm", "cm", "km", "kg", "ft", "inch", "mi", "ii", "jj", "ee", "dx", "dy", "dt", "xin", "sint",
  "ch", "sh", "th", "tg", "ctg", "cth", "cotg", "arctg", "lg", "lb", "cosec",
  "mean", "median", "fft", "lcm", "erf", "erfc", "bessel", "randomReal", "randomInteger",
  "approaches", "union", "asterisk", "divide", "infinity", "defint", "times", "prop",
  "diamond", "square", "lt", "lt=", "gt", "gt=", "ceil", "floor", "frac", "cbrt", "grad",
  "del", "TT", "AA", "EE", "!EE", "Re", "Im",
];

let shortcutTable: InlineShortcutDefinitions | null = null;

/** MathLive's table, pruned, with the note's own shorthands (`@a` → α,
 *  `sr` → ², `->` → →) on top. */
export function shortcuts(defaults: Readonly<InlineShortcutDefinitions>): InlineShortcutDefinitions {
  if (shortcutTable) return shortcutTable;
  const out: InlineShortcutDefinitions = { ...defaults };
  for (const key of PRUNED) delete out[key];
  for (const [key, name] of Object.entries(GREEK)) out[`@${key}`] = `\\${name}`;
  for (const [key, latex] of Object.entries(POWERS)) out[key] = latex.replace(/#\{1\}/, "#?").replace(/#\{0\}/, "");
  for (const [key, latex] of OPERATORS) if (!key.startsWith("\\")) out[key] = latex;
  shortcutTable = out;
  return out;
}

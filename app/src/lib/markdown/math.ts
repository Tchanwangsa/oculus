/** Clipboard type of a display formula's copy (a block field's, rendered
 *  markdown's): its source with the `$$` lines, for a paste outside maths
 *  (`components/documents/editor/live-preview/livePreview/edges.ts`). */
export const BLOCK_MATH_TYPE = "application/x-oculus-math-block";

/** Detect source delimiters before enabling remark-math or normalizing them.
 *  `\(` and `\[` count: the gate runs before [`normalizeMath`] rewrites them. */
const MATH = /\$|\\\(|\\\[/;
/** Code fences and inline code spans, left unchanged during normalization. */
const CODE = /(```[\s\S]*?```|~~~[\s\S]*?~~~|`[^`\n]*`)/g;

export function hasMath(text: string): boolean {
  // A regex cannot distinguish CommonMark code spans from escaped backticks;
  // leave that decision to the parser so a plugin gate never hides valid math.
  return MATH.test(text);
}

const DISPLAY = /\\\[([\s\S]+?)\\\]/g;
const INLINE = /\\\(([\s\S]+?)\\\)/g;

/**
 * Rewrite `\(…\)` / `\[…\]` to `$…$` / `$$…$$`. CommonMark treats `\(` as an
 * escaped `(` and eats the backslash before remark-math sees a delimiter.
 * Code spans and fences are left alone.
 */
export function normalizeMath(text: string): string {
  if (!hasMath(text)) return text;
  return text
    .split(CODE)
    .map((part, i) =>
      i % 2 === 1
        ? part
        : part.replace(DISPLAY, (_, m) => `$$${m}$$`).replace(INLINE, (_, m) => `$${m}$`),
    )
    .join("");
}

import { renderToString, type MathField } from "@/lib/maths";
import { HUG_KERN, huggedArrays } from "../../hugArrays";
import { MATH_ARRAYSTRETCH } from "../mathField/layout";

/** `tex`'s offset back in the source `huggedArrays` grew it from, or null
 *  inside an inserted kern. `at` holds the source offsets of the kerns. */
function unhugged(t: number, at: readonly number[]): number | null {
  const n = HUG_KERN.length;
  for (let k = 0; k < at.length; k++) {
    const start = at[k] + k * n;
    if (t <= start) return t - k * n;
    if (t < start + n) return null;
  }
  return t - at.length * n;
}

/** The source map of HTML rendered from hugged `tex`, moved back onto the
 *  source; a kern's own element loses its range. */
export function unhugMap(html: string, at: readonly number[]): string {
  if (!at.length) return html;
  return html.replace(/ data-s="(\d+)" data-e="(\d+)"/g, (_, s: string, e: string) => {
    const from = unhugged(Number(s), at);
    const to = unhugged(Number(e), at);
    return from == null || to == null || (from === to && s !== e) ? "" : ` data-s="${from}" data-e="${to}"`;
  });
}

/** The field's maths as the Live rendering draws it (`live-preview/widgets/
 *  math.ts`: `hugArrays`, `MATH_ARRAYSTRETCH`), with the source map on.
 *  Throws a `MathsTrap` when the engine traps on it. */
export function fieldHtml(source: string, display: boolean): string {
  const { tex, at } = huggedArrays(source);
  // A fresh macro table each time: KaTeX writes the source's `\def`s into it.
  const macros = { "\\arraystretch": String(MATH_ARRAYSTRETCH) };
  const html = renderToString(tex, { displayMode: display, sourceMap: true, strict: "ignore", throwOnError: true, macros });
  return unhugMap(html, at);
}

/**
 * A top-level row with nothing in it (an empty field, the line after a
 * trailing `\\`) draws no box at all, so the caret would have nowhere to
 * go: each gets an empty `katex-base` holding a zero-width `oc-empty-row`
 * marker at its offset, in its place among the `katex-newline`s.
 */
export function markEmptyRows(root: Element, field: MathField) {
  const html = root.querySelector(".katex-html");
  if (!html) return;
  const ranges = [...html.querySelectorAll("[data-s]")].map((el) => [
    Number(el.getAttribute("data-s")),
    Number(el.getAttribute("data-e")),
  ]);
  const breaks = [...html.children].filter((el) => el.classList.contains("katex-newline"));
  let row = 0;
  for (const slot of field.slots) {
    if (slot.parent != null) continue;
    const k = slot.row ?? row;
    row++;
    if (ranges.some(([s, e]) => s >= slot.from && e <= slot.to && (e > s || s === slot.from))) continue;
    const base = document.createElement("span");
    base.className = "katex-base";
    const marker = document.createElement("span");
    marker.className = "oc-empty-row";
    marker.dataset.s = marker.dataset.e = String(slot.from);
    base.append(marker);
    if (k === 0) html.prepend(base);
    else if (breaks[k - 1]) breaks[k - 1].after(base);
    else html.append(base);
  }
}

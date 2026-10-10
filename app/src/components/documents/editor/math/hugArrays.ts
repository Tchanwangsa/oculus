/** `\left[ \begin{array}…\end{array} \right]` drawn like `bmatrix`: an
 *  array keeps `\arraycolsep` outside its first and last columns (matrices
 *  drop it), which reads as a gap inside the brackets. Render-time only:
 *  the Live rendering and the field (`field/mathView`) both draw it so. */
const LEFT_BEFORE = /\\left\s*(?:\\[a-zA-Z]+|\\.|[^\s\\])\s*$/;
const BEGIN = "\\begin{array}";
const END = "\\end{array}";
export const HUG_KERN = "\\kern-0.5em";

/** `source` with a `HUG_KERN` either side of each such array, and the
 *  offsets of `source` where each kern went, ascending. */
export function huggedArrays(source: string): { tex: string; at: number[] } {
  const at: number[] = [];
  if (!source.includes(BEGIN)) return { tex: source, at };
  let out = "";
  let done = 0;
  for (let i = source.indexOf(BEGIN); i >= 0; i = source.indexOf(BEGIN, i + 1)) {
    if (i < done || !LEFT_BEFORE.test(source.slice(0, i))) continue;
    // The matching `\end{array}`, past any nested array.
    let depth = 0;
    let end = -1;
    for (let j = i; j < source.length; j++) {
      if (source.startsWith(BEGIN, j)) depth++;
      else if (source.startsWith(END, j) && --depth === 0) {
        end = j + END.length;
        break;
      }
    }
    if (end < 0 || !/^\s*\\right/.test(source.slice(end))) continue;
    out += `${source.slice(done, i)}${HUG_KERN}${source.slice(i, end)}${HUG_KERN}`;
    at.push(i, end);
    done = end;
  }
  return { tex: out + source.slice(done), at };
}

export function hugArrays(source: string): string {
  return huggedArrays(source).tex;
}

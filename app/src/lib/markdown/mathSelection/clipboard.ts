import { layoutBlock } from "@/components/documents/editor/math/field/mathField/serialize";
import { BLOCK_MATH_TYPE } from "../math";

/**
 * What copying the source range [from, to) of a formula writes, by type:
 * its TeX as `text/plain` and as `application/x-latex` (the note's paste
 * keeps it maths, `fieldLatexPaste`), and a display formula's also as
 * `$$` lines (`BLOCK_MATH_TYPE`). Nothing for an empty range.
 */
export function copied(source: string, [from, to]: readonly [number, number], display: boolean): [string, string][] {
  const latex = source.slice(from, to).trim();
  if (!latex) return [];
  const out: [string, string][] = [
    ["text/plain", latex],
    ["application/x-latex", latex],
  ];
  if (display) out.push([BLOCK_MATH_TYPE, `$$\n${layoutBlock(latex)}\n$$`]);
  return out;
}

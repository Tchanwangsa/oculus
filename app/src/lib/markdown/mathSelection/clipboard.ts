import { layoutBlock } from "@/components/documents/editor/math/field/mathField/serialize";
import type { BandSlot } from "@/lib/maths/geometry";
import { BLOCK_MATH_TYPE } from "../math";

/** What a copy reads of a formula's model (`MathField`). */
export interface CopySource {
  readonly source: string;
  readonly selected: readonly [number, number];
  readonly slots: readonly BandSlot[];
}

/**
 * Whether the selection [from, to) of `source` copies as a block: a display
 * formula taken whole, or a selection over more than one line, two of its
 * top-level rows or an array whose rows it holds (the model takes an array
 * whole once a selection crosses its cells, so its environment comes too).
 * Anything within one line copies inline.
 */
export function copiesAsBlock({ source, selected: [from, to], slots }: CopySource, display: boolean): boolean {
  if (display && source.slice(from, to).trim() === source.trim()) return true;
  const rows = slots.filter((s) => s.parent == null && s.kind !== "cell");
  const lines = rows.filter((r) => (r.from < to && r.to > from) || (r.from === r.to && from < r.from && r.from < to));
  if (lines.length > 1) return true;
  const cellRows = new Set(slots.filter((s) => s.kind === "cell" && s.from >= from && s.to <= to).map((s) => s.row));
  return cellRows.size > 1;
}

/**
 * What copying a formula's selection writes, by type: its TeX wrapped by
 * its shape as `text/plain` — `$…$`, or `$$` lines one row each
 * (`copiesAsBlock`) — the bare TeX as `application/x-latex` (the note's
 * paste keeps it maths, `fieldLatexPaste`), and a block's `$$` lines as
 * `BLOCK_MATH_TYPE`. Nothing for an empty range.
 */
export function copied(from: CopySource, display: boolean): [string, string][] {
  const [start, end] = from.selected;
  const latex = from.source.slice(start, end).trim();
  if (!latex) return [];
  if (!copiesAsBlock(from, display)) {
    return [
      ["text/plain", `$${latex}$`],
      ["application/x-latex", latex],
    ];
  }
  const block = `$$\n${layoutBlock(latex)}\n$$`;
  return [
    ["text/plain", block],
    ["application/x-latex", latex],
    [BLOCK_MATH_TYPE, block],
  ];
}

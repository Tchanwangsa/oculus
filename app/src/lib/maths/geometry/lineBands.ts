import { union, type Box } from "./box";

export interface LinePiece { box: Box; baseline: number }

/** Text and inline maths sharing a baseline share the tallest highlight.
 *  Separate horizontal runs stay separate, including bidi selections and
 *  gaps occupied by non-selectable controls. */
export function lineBands(pieces: readonly LinePiece[]): Box[] {
  const rows: { extent: Box; baseline: number; pieces: Box[] }[] = [];
  for (const piece of [...pieces].sort((a, b) => a.baseline - b.baseline)) {
    const row = rows.find(r => Math.abs(r.baseline - piece.baseline) <= 3);
    if (row) {
      row.extent = union(row.extent, piece.box);
      row.pieces.push(piece.box);
    } else rows.push({ extent: { ...piece.box }, baseline: piece.baseline, pieces: [piece.box] });
  }
  return rows.flatMap(row => {
    const runs: Box[] = [];
    for (const box of [...row.pieces].sort((a, b) => a.left - b.left)) {
      const last = runs[runs.length - 1];
      if (last && box.left <= last.right + 1) last.right = Math.max(last.right, box.right);
      else runs.push({ ...box, top: row.extent.top, bottom: row.extent.bottom });
    }
    return runs;
  });
}

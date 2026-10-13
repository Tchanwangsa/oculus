import { union, type Box } from "@/lib/maths/geometry";

export interface ProsePiece { box: Box; baseline: number }

/** Text and inline maths sharing a baseline share the tallest highlight.
 *  Baselines, rather than intersecting ink, keep wrapped lines separate. */
export function proseBands(pieces: readonly ProsePiece[]): Box[] {
  const rows: { box: Box; baseline: number }[] = [];
  for (const piece of [...pieces].sort((a, b) => a.baseline - b.baseline)) {
    const row = rows.find(r => Math.abs(r.baseline - piece.baseline) <= 2);
    if (row) row.box = union(row.box, piece.box);
    else rows.push({ box: { ...piece.box }, baseline: piece.baseline });
  }
  return rows.map(r => r.box);
}

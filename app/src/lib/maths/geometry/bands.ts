import { union, type Box } from "./box";
import type { Layout, MappedBox } from "./layout";

/** Unions boxes that overlap vertically, until none do: one band per
 *  visual line. */
function lines(inks: Box[]): Box[] {
  const out: Box[] = [];
  for (const ink of [...inks].sort((a, b) => a.top - b.top)) {
    if (ink.bottom <= ink.top) continue;
    const row = out.find((r) => ink.top < r.bottom && ink.bottom > r.top);
    if (row) Object.assign(row, union(row, ink));
    else out.push({ ...ink });
  }
  // Merging can grow a band into the next one.
  for (let i = 0; i < out.length; i++) {
    for (let j = i + 1; j < out.length; j++) {
      if (out[j].top < out[i].bottom && out[j].bottom > out[i].top) {
        out[i] = union(out[i], out[j]);
        out.splice(j, 1);
        j = i;
      }
    }
  }
  return out;
}

/**
 * The selection highlight of the source range [from, to): one band per
 * visual row, never a box per glyph. The outermost elements inside the
 * range, their ink grouped by the top-level row (`rows`, a display's lines)
 * holding them and, within one, where it overlaps vertically; each group
 * one band from its leftmost to its rightmost ink. Bands of rows drawn
 * overlapping (a tall fraction over a matrix) are cut where they meet.
 */
export function bands(layout: Layout, from: number, to: number, rows: readonly { from: number; to: number }[] = []): Box[] {
  if (to <= from) return [];
  const outer: MappedBox[] = [];
  for (const item of layout.items) {
    const inside = item.placeholder ? item.from >= from && item.from < to : item.from >= from && item.to <= to && item.to > item.from;
    if (!inside) continue;
    const last = outer[outer.length - 1];
    // Document order: an element inside the last one taken comes right after it.
    if (last && last.from <= item.from && item.to <= last.to && !last.placeholder) continue;
    outer.push(item);
  }
  const groups = new Map<number, Box[]>();
  for (const item of outer) {
    const row = rows.findIndex((r) => item.from >= r.from && item.to <= r.to);
    groups.set(row, [...(groups.get(row) ?? []), item.ink]);
  }
  const out = [...groups.values()].flatMap(lines).sort((a, b) => a.top - b.top);
  for (let i = 1; i < out.length; i++) {
    const [above, below] = [out[i - 1], out[i]];
    if (below.top < above.bottom && below.bottom > above.bottom) {
      const cut = (below.top + above.bottom) / 2;
      above.bottom = cut;
      below.top = cut;
    }
  }
  return out;
}

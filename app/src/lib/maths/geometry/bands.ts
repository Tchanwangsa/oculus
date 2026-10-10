import { union, type Box } from "./box";
import type { Layout, MappedBox } from "./layout";

/** The slots `bands` reads (`FieldSlot`'s): a top-level row has no
 *  parent; an array cell is `kind: "cell"` with its `row`. */
export interface BandSlot {
  from: number;
  to: number;
  parent?: number | null;
  kind?: string;
  row?: number;
}

/** A delimiter left beside an array's cells (a matrix's bracket, `cases`'
 *  brace) is drawn when it is wider than this fraction of its font size;
 *  the array's own edge padding is narrower. */
const DELIMITER_EM = 0.15;

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

const within = (a: { from: number; to: number }, b: { from: number; to: number }) => a.from >= b.from && a.to <= b.to;

/** The outermost elements inside [from, to), each as one unit, except a
 *  structure holding array rows of its own (`split`), whose parts are taken
 *  instead. */
function outermost(layout: Layout, from: number, to: number, cells: readonly BandSlot[]) {
  const taken: MappedBox[] = [];
  const split: MappedBox[] = [];
  const rowsIn = (item: MappedBox) => new Set(cells.filter((c) => within(c, item)).map((c) => c.row)).size;
  let last: MappedBox | undefined;
  for (const item of layout.items) {
    const inside = item.placeholder ? item.from >= from && item.from < to : item.from >= from && item.to <= to && item.to > item.from;
    if (!inside) continue;
    // Document order: an element inside the last one taken comes right after it.
    if (last && last.from <= item.from && item.to <= last.to && !last.placeholder) continue;
    if (!item.placeholder && rowsIn(item) > 1) split.push(item);
    else taken.push((last = item));
  }
  return { taken, split };
}

/**
 * The selection highlight of the source range [from, to): bands over the
 * selected atoms, never a box per glyph nor one box over a whole formula.
 * The outermost elements inside the range, grouped by the line holding
 * them, and within a group where they overlap vertically, each group one
 * band from its leftmost to its rightmost ink. A line is a top-level row
 * (`slots` without a parent, a display's lines) or one row of an array
 * whose rows are all selected: such an array (`cases`, a matrix) gives a
 * band per row, its delimiters theirs beside them, and the atoms before and
 * after it theirs. Bands drawn overlapping (a tall fraction over a matrix)
 * are cut where they meet.
 */
export function bands(layout: Layout, from: number, to: number, slots: readonly BandSlot[] = []): Box[] {
  if (to <= from) return [];
  const cells = slots.filter((s) => s.kind === "cell");
  const rows = slots.filter((s) => s.parent == null && s.kind !== "cell");
  const { taken, split } = outermost(layout, from, to, cells);

  // The line an element (or a split structure's delimiter) is drawn on: the
  // row of the innermost split array holding it, else its top-level row.
  const scopeOf = (r: { from: number; to: number }, self?: MappedBox) => {
    const around = split.filter((s) => s !== self && within(r, s) && !(self && within(s, self)));
    const array = around[around.length - 1];
    const cell = array && cells.filter((c) => within(c, array) && within(r, c)).pop();
    if (array && cell) return { scope: `a${array.from}:${array.to}`, key: `a${array.from}:${array.to}:${cell.row}` };
    const owner = array ? `a${array.from}:${array.to}` : `r${rows.findIndex((row) => within(r, row))}`;
    return { scope: owner, key: owner };
  };

  type Entry = { at: number; order: number; scope: string; key: string; ink: Box };
  const entries: Entry[] = [];
  taken.forEach((item, i) => entries.push({ at: item.from, order: i, ...scopeOf(item), ink: item.ink }));
  for (const s of split) {
    const parts = taken.filter((t) => within(t, s));
    if (!parts.length) continue;
    const left = Math.min(...parts.map((p) => p.ink.left));
    const right = Math.max(...parts.map((p) => p.ink.right));
    const first = taken.indexOf(parts[0]);
    const lastPart = taken.indexOf(parts[parts.length - 1]);
    const min = DELIMITER_EM * s.size;
    const where = scopeOf(s, s);
    if (left - s.ink.left > min) entries.push({ at: s.from, order: first - 0.5, ...where, ink: { ...s.ink, right: left } });
    if (s.ink.right - right > min) entries.push({ at: s.to, order: lastPart + 0.5, ...where, ink: { ...s.ink, left: right } });
  }
  entries.sort((a, b) => a.order - b.order);

  // A line's run ends where something drawn on another line's scope comes
  // between, so the atoms either side of an array never join over it. An
  // array's own rows interleave (KaTeX draws it column by column).
  const groups = new Map<string, Box[]>();
  const open = new Map<string, { scope: string; group: string }>();
  let seq = 0;
  for (const e of entries) {
    for (const [key, run] of open) if (run.scope !== e.scope) open.delete(key);
    let run = open.get(e.key);
    if (!run) open.set(e.key, (run = { scope: e.scope, group: `${e.key}#${seq++}` }));
    groups.set(run.group, [...(groups.get(run.group) ?? []), e.ink]);
  }

  const out = [...groups.values()].flatMap(lines).sort((a, b) => a.top - b.top);
  for (let i = 1; i < out.length; i++) {
    for (let j = 0; j < i; j++) {
      const [above, below] = [out[j], out[i]];
      const sideBySide = below.left >= above.right || below.right <= above.left;
      if (!sideBySide && below.top < above.bottom && below.bottom > above.bottom) {
        const cut = (below.top + above.bottom) / 2;
        above.bottom = cut;
        below.top = cut;
      }
    }
  }
  return out;
}

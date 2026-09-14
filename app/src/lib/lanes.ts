/**
 * Laying overlapping things out side by side, for any view that draws a set of
 * spans in one track.
 *
 * Lifted out of `app/src/components/calendar/WeekView.tsx`, which was the only
 * caller until the project timeline needed the same packing. Generic over the
 * item rather than tied to `CalEvent`, because the two callers measure a span
 * differently — a class has a start and a duration in minutes, a task bar has
 * a left and a width in pixels — and the packing itself never needs to know
 * which. The numbers only have to share a unit with each other.
 */

/** A span on whatever axis the caller is packing: milliseconds, pixels, either.
 *  `end` is exclusive in the sense that touching spans do not overlap. */
export interface LaneSpan {
  start: number;
  end: number;
}

export interface Lane<T> {
  item: T;
  /** 0-based lane within the item's cluster. */
  lane: number;
  /** How many lanes that cluster needed — the denominator the item's width is
   *  drawn against. */
  of: number;
}

/** Lay overlapping items out side by side: cluster anything that touches,
 *  then give each item the first lane free at its start. */
export function packLanes<T>(items: T[], span: (item: T) => LaneSpan): Lane<T>[] {
  const sorted = [...items].sort((a, b) => span(a).start - span(b).start);
  const out: Lane<T>[] = [];

  let cluster: T[] = [];
  let clusterEnd = -Infinity;

  const flush = () => {
    if (cluster.length === 0) return;
    const laneEnds: number[] = [];
    const placed = cluster.map((e) => {
      const { start, end } = span(e);
      let lane = laneEnds.findIndex((t) => t <= start);
      if (lane === -1) lane = laneEnds.length;
      laneEnds[lane] = end;
      return { item: e, lane };
    });
    for (const p of placed) out.push({ ...p, of: laneEnds.length });
    cluster = [];
    clusterEnd = -Infinity;
  };

  for (const e of sorted) {
    const { start, end } = span(e);
    if (start >= clusterEnd) flush();
    cluster.push(e);
    clusterEnd = Math.max(clusterEnd, end);
  }
  flush();
  return out;
}

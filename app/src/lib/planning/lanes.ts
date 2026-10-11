/** A span in any one unit (ms, px). Touching spans do not overlap. */
export interface LaneSpan {
  start: number;
  end: number;
}

export interface Lane<T> {
  item: T;
  /** 0-based lane within the item's cluster. */
  lane: number;
  /** Lanes in that cluster — the denominator for the item's width. */
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

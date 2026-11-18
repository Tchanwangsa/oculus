import type { Point } from '../model';
import { STEP, flip, isHorizontal, type Heading } from './headings';
import type { Lane } from './lanes';

export interface GridSearch {
  xs: readonly number[];
  ys: readonly number[];
  from: Point;
  to: Point;
  /** The direction the route leaves `from` in; with `strictStart` its first move must take it. */
  startHeading: Heading;
  /** The direction pointing back out of `to`; the route arrives travelling the opposite way, and must with `strictEnd`. */
  endHeading: Heading;
  strictStart: boolean;
  strictEnd: boolean;
  blocked: (a: Point, b: Point) => boolean;
  /** Fixed segments' lines, which the route must run along in this order. */
  lanes?: readonly Lane[];
}

/**
 * What meeting a lane's line only by crossing it costs, in bends: running along it wins unless that takes three more
 * bends, which is when the ends have come level across it and the segment would be a detour of its own.
 */
export const CROSSING = 2.5;

/** A lower bound on the bends still needed to be at `to` travelling `arrive`, moving `dir` from `p`. Consistent, so A* never reopens a state. */
function bendsLeft(dir: Heading, p: Point, to: Point, arrive: Heading): number {
  if (dir === flip(arrive)) return 2;
  if (dir !== arrive) return 1;
  const ahead = isHorizontal(dir) ? p.y === to.y && (to.x - p.x) * STEP[dir].x >= 0 : p.x === to.x && (to.y - p.y) * STEP[dir].y >= 0;
  return ahead ? 0 : 2;
}

interface Entry { bends: number; length: number; seq: number; state: number }
const before = (a: Entry, b: Entry) => a.bends !== b.bends ? a.bends < b.bends : a.length !== b.length ? a.length < b.length : a.seq < b.seq;

function push(heap: Entry[], entry: Entry) {
  let i = heap.push(entry) - 1;
  while (i > 0) {
    const parent = (i - 1) >> 1;
    if (!before(heap[i], heap[parent])) break;
    [heap[i], heap[parent]] = [heap[parent], heap[i]]; i = parent;
  }
}

function pop(heap: Entry[]): Entry {
  const top = heap[0], last = heap.pop()!;
  if (heap.length) {
    heap[0] = last;
    for (let i = 0; ;) {
      const l = 2 * i + 1, r = l + 1;
      let m = i;
      if (l < heap.length && before(heap[l], heap[m])) m = l;
      if (r < heap.length && before(heap[r], heap[m])) m = r;
      if (m === i) break;
      [heap[i], heap[m]] = [heap[m], heap[i]]; i = m;
    }
  }
  return top;
}

/**
 * A* over the grid's intersections from `from` to `to`, minimising bends first and Manhattan length second (compared
 * as a pair, so a bend always outweighs any detour). A state is a node, the direction it was entered in (what a bend
 * is counted against) and how many lanes the route has run along; a route never doubles back on itself, and reaches
 * `to` only once it has taken every lane. A lane may be crossed instead (including at `from`), at CROSSING bends. With lanes the bend heuristic is dropped, since it would not be consistent
 * across a lane. Returns the nodes visited, or null.
 */
export function gridRoute(search: GridSearch): Point[] | null {
  const { xs, ys, from, to, startHeading, endHeading, strictStart, strictEnd, blocked, lanes = [] } = search;
  const cols = xs.length, nodes = cols * ys.length;
  const index = (p: Point) => xs.includes(p.x) && ys.includes(p.y) ? ys.indexOf(p.y) * cols + xs.indexOf(p.x) : -1;
  const start = index(from), target = index(to);
  if (start < 0 || target < 0) return null;
  const arrive = flip(endHeading), goal = (lanes.length + 1) * nodes * 4;
  const bends = new Float64Array(goal + 1).fill(Infinity), lengths = new Float64Array(goal + 1).fill(Infinity);
  const parent = new Int32Array(goal + 1).fill(-1), closed = new Uint8Array(goal + 1);
  const heap: Entry[] = [];
  let seq = 0;
  const at = (node: number): Point => ({ x: xs[node % cols], y: ys[Math.floor(node / cols)] });
  const stateOf = (stage: number, node: number, dir: Heading) => (stage * nodes + node) * 4 + dir;
  const nodeOf = (state: number) => (state >> 2) % nodes;
  const relax = (state: number, b: number, l: number, prev: number) => {
    if (closed[state] || b > bends[state] || (b === bends[state] && l >= lengths[state])) return;
    bends[state] = b; lengths[state] = l; parent[state] = prev;
    const p = state === goal ? to : at(nodeOf(state));
    const hb = state === goal || lanes.length ? 0 : bendsLeft((state & 3) as Heading, p, to, arrive);
    push(heap, { bends: b + hb, length: l + Math.abs(to.x - p.x) + Math.abs(to.y - p.y), seq: seq++, state });
  };
  const onLine = (lane: Lane | undefined, p: Point) => !!lane && (lane.horizontal ? p.y === lane.at : p.x === lane.at);
  relax(stateOf(0, start, startHeading), 0, 0, -1);
  if (onLine(lanes[0], from)) relax(stateOf(1, start, startHeading), CROSSING, 0, -1);
  while (heap.length) {
    const { state } = pop(heap);
    if (closed[state]) continue;
    closed[state] = 1;
    if (state === goal) {
      const route: Point[] = [];
      for (let s = parent[goal]; s >= 0; s = parent[s]) route.unshift(at(nodeOf(s)));
      return route;
    }
    const node = nodeOf(state), stage = Math.floor((state >> 2) / nodes), dir = (state & 3) as Heading, p = at(node);
    const col = node % cols, row = Math.floor(node / cols), lane = lanes[stage];
    for (let i = 0; i < 4; i++) {
      const d = i as Heading;
      if (d === flip(dir) || (strictStart && node === start && d !== startHeading)) continue;
      const c = col + STEP[d].x, r = row + STEP[d].y;
      if (c < 0 || c >= cols || r < 0 || r >= ys.length) continue;
      const next = r * cols + c, q = at(next);
      if (blocked(p, q)) continue;
      const b = bends[state] + (d === dir ? 0 : 1), l = lengths[state] + Math.abs(q.x - p.x) + Math.abs(q.y - p.y);
      const meets = onLine(lane, q), along = meets && lane.horizontal === isHorizontal(d);
      const steps: [number, number][] = along ? [[stage + 1, b]] : meets ? [[stage, b], [stage + 1, b + CROSSING]] : [[stage, b]];
      for (const [reached, cost] of steps) {
        const moved = stateOf(reached, next, d);
        relax(moved, cost, l, state);
        if (next === target && reached === lanes.length && d !== endHeading && (!strictEnd || d === arrive)) relax(goal, cost + (d === arrive ? 0 : 1), l, moved);
      }
    }
  }
  return null;
}

import { finite, type Vec3 } from "./vec";

export interface Grid {
  /** `(samples + 1)²` vertices, row by row along `u`. */
  positions: Float32Array;
  /** Two triangles for each cell whose four corners are finite. */
  indices: Uint32Array;
  /** Whether each vertex is a real sample. */
  real: boolean[];
}

/**
 * A `samples` × `samples` grid of `xyz` over `u` × `v`. A sample that isn't
 * finite leaves a hole: no cell touching it is drawn, and its vertex copies
 * a real one so the mesh's bounds stay the surface's own.
 */
export function surfaceGrid(
  xyz: (u: number, v: number) => Vec3,
  u: [number, number],
  v: [number, number],
  samples: number,
): Grid {
  const n = Math.max(1, Math.floor(samples));
  const side = n + 1;
  const points: (Vec3 | null)[] = [];
  for (let j = 0; j <= n; j++) {
    const vv = v[0] + ((v[1] - v[0]) * j) / n;
    for (let i = 0; i <= n; i++) {
      const p = xyz(u[0] + ((u[1] - u[0]) * i) / n, vv);
      points.push(Array.isArray(p) && p.length >= 3 && finite(p) ? [p[0], p[1], p[2]] : null);
    }
  }
  const stand = points.find((p) => p != null) ?? [0, 0, 0];
  const positions = new Float32Array(side * side * 3);
  points.forEach((p, k) => positions.set(p ?? stand, k * 3));
  const cells: number[] = [];
  for (let j = 0; j < n; j++) {
    for (let i = 0; i < n; i++) {
      const a = j * side + i;
      const b = a + 1;
      const c = a + side;
      const d = c + 1;
      if (points[a] && points[b] && points[c] && points[d]) cells.push(a, b, d, a, d, c);
    }
  }
  return { positions, indices: new Uint32Array(cells), real: points.map((p) => p != null) };
}

/** The grid's lines along `u` and along `v`, `lines` of each, as polylines
 *  broken at holes (for a wire surface). */
export function gridLines(grid: Grid, samples: number, lines: number): Vec3[][] {
  const n = Math.max(1, Math.floor(samples));
  const side = n + 1;
  const every = Math.max(1, Math.round(n / Math.max(1, lines)));
  const at = (i: number, j: number): Vec3 | null => {
    const k = j * side + i;
    if (!grid.real[k]) return null;
    return [grid.positions[k * 3], grid.positions[k * 3 + 1], grid.positions[k * 3 + 2]];
  };
  const out: Vec3[][] = [];
  const trace = (point: (s: number) => Vec3 | null) => {
    let run: Vec3[] = [];
    for (let s = 0; s <= n; s++) {
      const p = point(s);
      if (p) run.push(p);
      else {
        if (run.length > 1) out.push(run);
        run = [];
      }
    }
    if (run.length > 1) out.push(run);
  };
  for (let k = 0; k <= n; k += every) {
    trace((s) => at(s, k));
    trace((s) => at(k, s));
  }
  return out;
}

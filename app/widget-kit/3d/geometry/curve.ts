import { finite, type Vec3 } from "./vec";

/**
 * `xyz` sampled `samples` times across `domain`, cut into the runs between
 * points that aren't finite, so a curve is never joined
 * across a gap. A run of one point draws nothing and is left out.
 */
export function curvePieces(xyz: (t: number) => Vec3, domain: [number, number], samples: number): Vec3[][] {
  const [a, b] = domain;
  const n = Math.max(1, Math.floor(samples));
  const pieces: Vec3[][] = [];
  let run: Vec3[] = [];
  const close = () => {
    if (run.length > 1) pieces.push(run);
    run = [];
  };
  for (let i = 0; i <= n; i++) {
    const t = a + ((b - a) * i) / n;
    const v = xyz(t);
    if (Array.isArray(v) && v.length >= 3 && finite(v)) run.push([v[0], v[1], v[2]]);
    else close();
  }
  close();
  return pieces;
}

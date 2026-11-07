import { add, cross, dot, length, perpendicular, scale, sub, unit, type Vec3 } from "./vec";

export interface Mesh {
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
}

/** Around each ring: round enough for a line a few pixels thick. */
const SIDES = 8;

const EMPTY: Mesh = { positions: new Float32Array(), normals: new Float32Array(), indices: new Uint32Array() };

/**
 * A tube of `radius` through `points`: how the kit draws a line, since WebGL
 * line widths are always 1px in WebKit. Rings follow the path by parallel
 * transport, so the tube never twists. A path that returns to its start is
 * joined round; any other gets flat caps. Repeated points are skipped; fewer
 * than two distinct points draw nothing.
 */
export function tube(path: Vec3[], radius: number, sides = SIDES): Mesh {
  const points: Vec3[] = [];
  for (const p of path) if (!points.length || length(sub(p, points[points.length - 1])) > 1e-9) points.push(p);
  if (points.length < 2 || !(radius > 0)) return EMPTY;
  const closed = points.length > 3 && length(sub(points[0], points[points.length - 1])) < 1e-7;
  if (closed) points.pop();
  const n = points.length;
  const at = (i: number) => points[closed ? (i + n) % n : Math.max(0, Math.min(n - 1, i))];

  const tangents: Vec3[] = [];
  for (let i = 0; i < n; i++) tangents.push(unit(sub(at(i + 1), at(i - 1))) ?? unit(sub(at(i + 1), at(i))) ?? unit(sub(at(i), at(i - 1)))!);
  const normals: Vec3[] = [perpendicular(tangents[0])];
  for (let i = 1; i < n; i++) {
    const prev = normals[i - 1];
    normals.push(unit(sub(prev, scale(tangents[i], dot(prev, tangents[i])))) ?? perpendicular(tangents[i]));
  }

  const caps = closed ? 0 : 2;
  const vertexCount = n * sides + caps * (sides + 1);
  const pos = new Float32Array(vertexCount * 3);
  const nor = new Float32Array(vertexCount * 3);
  const idx: number[] = [];
  let v = 0;
  const put = (p: Vec3, q: Vec3) => {
    pos.set(p, v * 3);
    nor.set(q, v * 3);
    return v++;
  };
  const around = (i: number, s: number): Vec3 => {
    const a = (2 * Math.PI * s) / sides;
    const b = cross(tangents[i], normals[i]);
    return add(scale(normals[i], Math.cos(a)), scale(b, Math.sin(a)));
  };

  for (let i = 0; i < n; i++)
    for (let s = 0; s < sides; s++) {
      const dir = around(i, s);
      put(add(points[i], scale(dir, radius)), dir);
    }
  // Counterclockwise seen from outside: (ring i, θ), (i, θ+1), (i+1, θ).
  const segments = closed ? n : n - 1;
  for (let i = 0; i < segments; i++) {
    const r0 = i * sides;
    const r1 = ((i + 1) % n) * sides;
    for (let s = 0; s < sides; s++) {
      const s1 = (s + 1) % sides;
      idx.push(r0 + s, r0 + s1, r1 + s, r0 + s1, r1 + s1, r1 + s);
    }
  }
  if (!closed) {
    for (const [i, facing] of [[0, -1], [n - 1, 1]] as const) {
      const normal = scale(tangents[i], facing);
      const centre = put(points[i], normal);
      const ring = v;
      for (let s = 0; s < sides; s++) put(add(points[i], scale(around(i, s), radius)), normal);
      for (let s = 0; s < sides; s++) {
        const s1 = (s + 1) % sides;
        if (facing < 0) idx.push(centre, ring + s1, ring + s);
        else idx.push(centre, ring + s, ring + s1);
      }
    }
  }
  return { positions: pos, normals: nor, indices: new Uint32Array(idx) };
}

/** Several meshes as one, so a set of lines is one draw. */
export function merge(meshes: Mesh[]): Mesh {
  const vertices = meshes.reduce((n, m) => n + m.positions.length, 0);
  const indices = meshes.reduce((n, m) => n + m.indices.length, 0);
  const out: Mesh = { positions: new Float32Array(vertices), normals: new Float32Array(vertices), indices: new Uint32Array(indices) };
  let v = 0;
  let i = 0;
  for (const m of meshes) {
    out.positions.set(m.positions, v);
    out.normals.set(m.normals, v);
    for (let k = 0; k < m.indices.length; k++) out.indices[i + k] = m.indices[k] + v / 3;
    v += m.positions.length;
    i += m.indices.length;
  }
  return out;
}

import type { Vec3 } from "./vec";

/** Points per ring: round at any size the frame draws. */
const RING_SEGMENTS = 96;

/**
 * A wire sphere's rings, z up: `latitudes` circles of latitude between the
 * poles, evenly spaced in angle (the equator among them when odd), and
 * `meridians` half circles from pole to pole, evenly round. A latitude is a
 * closed polyline (its last point is its first); a meridian ends at the poles.
 */
export function sphereRings(center: Vec3, radius: number, latitudes = 5, meridians = 12): Vec3[][] {
  const [cx, cy, cz] = center;
  const rings: Vec3[][] = [];
  for (let k = 1; k <= latitudes; k++) {
    const polar = (Math.PI * k) / (latitudes + 1);
    const z = cz + radius * Math.cos(polar);
    const r = radius * Math.sin(polar);
    const ring: Vec3[] = [];
    for (let i = 0; i <= RING_SEGMENTS; i++) {
      const a = (2 * Math.PI * i) / RING_SEGMENTS;
      ring.push([cx + r * Math.cos(a), cy + r * Math.sin(a), z]);
    }
    rings.push(ring);
  }
  for (let m = 0; m < meridians; m++) {
    const a = (2 * Math.PI * m) / meridians;
    const half: Vec3[] = [];
    for (let i = 0; i <= RING_SEGMENTS / 2; i++) {
      const polar = (2 * Math.PI * i) / RING_SEGMENTS;
      const r = radius * Math.sin(polar);
      half.push([cx + r * Math.cos(a), cy + r * Math.sin(a), cz + radius * Math.cos(polar)]);
    }
    rings.push(half);
  }
  return rings;
}

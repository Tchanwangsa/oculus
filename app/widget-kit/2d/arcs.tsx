import { useTransformContext, vec } from "mafs";
import { paint, type Colour } from "../colors";
import { Label } from "./Label";
import type { Stroke } from "./lines";

type Vec = [number, number];

const TAU = 2 * Math.PI;

/** Maths to the SVG's pixel space, and its scale on each axis (y is negative). */
function usePixels() {
  const { viewTransform, userTransform } = useTransformContext();
  const m = vec.matrixMult(viewTransform, userTransform);
  return { px: (p: Vec): Vec => vec.transform(p, m), sx: m[0], sy: m[4] };
}

/** An SVG path along an ellipse about pixel `c` (radii `rx`, `ry`), from
 *  `start` by `sweep` radians, counterclockwise as seen when positive. Split
 *  into quarter turns or less, so no arc command is ambiguous. */
function arcPath(c: Vec, rx: number, ry: number, start: number, sweep: number, fromCenter = false): string {
  const at = (a: number): Vec => [c[0] + rx * Math.cos(a), c[1] - ry * Math.sin(a)];
  const n = Math.max(1, Math.ceil(Math.abs(sweep) / (Math.PI / 2) - 1e-9));
  const [x0, y0] = at(start);
  let d = fromCenter ? `M ${c[0]} ${c[1]} L ${x0} ${y0}` : `M ${x0} ${y0}`;
  for (let k = 1; k <= n; k++) {
    const [x, y] = at(start + (sweep * k) / n);
    d += ` A ${rx} ${ry} 0 0 ${sweep > 0 ? 0 : 1} ${x} ${y}`;
  }
  return fromCenter ? `${d} Z` : d;
}

const dash = (style: Stroke["style"]) => (style === "dashed" ? "var(--mafs-line-stroke-dash-style)" : undefined);

/**
 * Part of a circle about `center` of `radius` (maths units), from angle
 * `from` to `to` in radians, counterclockwise from the x-axis; `to` below
 * `from` runs clockwise.
 */
export function Arc({
  center,
  radius,
  from,
  to,
  color,
  weight = 2,
  style,
  opacity = 1,
}: Stroke & { center: Vec; radius: number; from: number; to: number }) {
  const { px, sx, sy } = usePixels();
  const d = arcPath(px(center), Math.abs(radius * sx), Math.abs(radius * sy), from, to - from);
  return (
    <path
      d={d}
      fill="none"
      strokeLinecap="round"
      style={{ stroke: paint(color, "foreground"), strokeWidth: weight, strokeOpacity: opacity, strokeDasharray: dash(style) }}
    />
  );
}

/**
 * The angle at `vertex` between the rays to `from` and to `to`, drawn as a
 * small wedge `radius` CSS px across (a square corner when it is right).
 * The smaller of the two angles, unless `directed`, which measures
 * counterclockwise from `from` to `to` and may pass a half turn. `label` is
 * TeX set just outside the wedge.
 */
export function Angle({
  vertex,
  from,
  to,
  radius = 26,
  directed = false,
  label,
  color,
  weight = 1.5,
  fillOpacity = 0.15,
}: {
  vertex: Vec;
  from: Vec;
  to: Vec;
  radius?: number;
  directed?: boolean;
  label?: string;
  color?: Colour;
  weight?: number;
  fillOpacity?: number;
}) {
  const { px, sx, sy } = usePixels();
  const paintColour = paint(color, "foreground");
  const v = px(vertex);
  // Screen angles, y up, so the wedge matches the rays as drawn.
  const screen = (p: Vec) => {
    const [x, y] = px(p);
    return Math.atan2(v[1] - y, x - v[0]);
  };
  const a1 = screen(from);
  let sweep = (((screen(to) - a1) % TAU) + TAU) % TAU;
  if (!directed && sweep > Math.PI) sweep -= TAU;
  const maths = (p: Vec) => Math.atan2(p[1] - vertex[1], p[0] - vertex[0]);
  const between = Math.abs((((maths(to) - maths(from)) % TAU) + TAU) % TAU);
  const right = Math.abs(between - Math.PI / 2) < 1e-6 || Math.abs(between - (3 * Math.PI) / 2) < 1e-6;
  const style = { stroke: paintColour, strokeWidth: weight, fill: paintColour, fillOpacity };

  let shape;
  if (right && Math.abs(Math.abs(sweep) - Math.PI / 2) < 0.2) {
    const s = radius * 0.6;
    const u = (a: number): Vec => [Math.cos(a) * s, -Math.sin(a) * s];
    const [ux, uy] = u(a1);
    const [wx, wy] = u(a1 + sweep);
    shape = <path d={`M ${v[0]} ${v[1]} L ${v[0] + ux} ${v[1] + uy} L ${v[0] + ux + wx} ${v[1] + uy + wy} L ${v[0] + wx} ${v[1] + wy} Z`} style={style} />;
  } else {
    shape = <path d={arcPath(v, radius, radius, a1, sweep, true)} style={style} />;
  }

  let text = null;
  if (label) {
    const mid = a1 + sweep / 2;
    const out = radius + 12;
    const at: Vec = [vertex[0] + (Math.cos(mid) * out) / sx, vertex[1] + (Math.sin(mid) * out) / -sy];
    text = <Label at={at} color={color ?? "foreground"} tex={label} />;
  }
  return (
    <g className="o-plane-angle">
      {shape}
      {text}
    </g>
  );
}

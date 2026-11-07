import type { Colour } from "../colors";
import { curvePieces } from "./geometry/curve";
import type { Vec3 } from "./geometry/vec";
import { Lines } from "./mesh";
import { LINE_RADIUS, useThickness } from "./scene";

/**
 * A space curve `xyz(t)` over `domain`, `samples` points long, drawn as a
 * tube. Points that aren't finite break it: the pieces either side are
 * never joined.
 */
export function Curve3D({
  xyz,
  domain,
  samples = 200,
  color = "chart-1",
  weight,
}: {
  xyz: (t: number) => Vec3;
  domain: [number, number];
  samples?: number;
  color?: Colour;
  weight?: number;
}) {
  const thickness = useThickness(LINE_RADIUS, weight);
  return <Lines paths={curvePieces(xyz, domain, samples)} radius={thickness} color={color} />;
}

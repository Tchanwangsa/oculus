import type { Colour } from "../colors";
import { arcPoints } from "./geometry/arc";
import type { Vec3 } from "./geometry/vec";
import { Lines } from "./mesh";
import { LINE_RADIUS, useThickness } from "./scene";

/**
 * The shorter arc around `center` from direction `from` to direction `to`
 * (vectors from the centre), `radius` from it (default `|from|`): the angle
 * between two vectors. Opposite directions turn about z (about x when
 * `from` is vertical); the same direction draws nothing.
 */
export function Arc3D({
  center = [0, 0, 0],
  from,
  to,
  radius,
  color = "foreground",
  weight,
}: {
  center?: Vec3;
  from: Vec3;
  to: Vec3;
  radius?: number;
  color?: Colour;
  weight?: number;
}) {
  const thickness = useThickness(LINE_RADIUS, weight);
  return <Lines paths={[arcPoints(center, from, to, radius)]} radius={thickness} color={color} />;
}

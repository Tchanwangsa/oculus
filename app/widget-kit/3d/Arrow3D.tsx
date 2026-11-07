import { useMemo } from "react";
import { Quaternion, Vector3 } from "three";
import type { Colour } from "../colors";
import { themeColour } from "./colour";
import { arrowParts } from "./geometry/arrow";
import { add, scale, type Vec3 } from "./geometry/vec";
import { FlatMaterial, Lines } from "./mesh";
import { ARROW_HEAD, ARROW_RADIUS, useThickness } from "./scene";

const UP = new Vector3(0, 1, 0);

/**
 * An arrow from `tail` to `tip`: a shaft and a cone head that grows with a
 * short arrow and stops growing past `ARROW_HEAD` (times `weight`). A zero
 * length draws nothing.
 */
export function Arrow3D({
  tail = [0, 0, 0],
  tip,
  color = "foreground",
  weight,
}: {
  tail?: Vec3;
  tip: Vec3;
  color?: Colour;
  weight?: number;
}) {
  const colour = themeColour(color, "foreground");
  const radius = useThickness(ARROW_RADIUS, weight);
  const maxHead = useThickness(ARROW_HEAD, weight);
  const parts = arrowParts(tail, tip, maxHead);
  const direction = parts?.direction;
  const turn = useMemo(
    () => (direction ? new Quaternion().setFromUnitVectors(UP, new Vector3(...direction)) : null),
    [direction?.[0], direction?.[1], direction?.[2]],
  );
  if (!parts || !turn) return null;
  // Never wider than the head, so a short arrow is still an arrow.
  const shaft = Math.min(radius, parts.headRadius * 0.5);
  return (
    <group>
      <Lines paths={[[tail, parts.neck]]} radius={shaft} color={color} />
      <mesh position={add(parts.neck, scale(parts.direction, parts.headLength / 2))} quaternion={turn}>
        <coneGeometry args={[parts.headRadius, parts.headLength, 24]} />
        <FlatMaterial color={colour} />
      </mesh>
    </group>
  );
}

import type { Colour } from "../colors";
import { themeColour } from "./colour";
import { sphereRings } from "./geometry/sphere";
import type { Vec3 } from "./geometry/vec";
import { Lines, SolidMaterial } from "./mesh";
import { LINE_RADIUS, useThickness } from "./scene";

/**
 * A sphere of `radius` about `center`: solid and lit, or with `wire` its
 * circles of latitude and longitude instead (z is the polar axis). Below
 * full `opacity` a solid one lets what is inside show.
 */
export function Sphere({
  center = [0, 0, 0],
  radius = 1,
  color = "brand",
  opacity = 1,
  wire = false,
}: {
  center?: Vec3;
  radius?: number;
  color?: Colour;
  opacity?: number;
  wire?: boolean;
}) {
  const colour = themeColour(color, "brand");
  // Rings are finer than a line: they are guides on the surface.
  const thickness = useThickness(LINE_RADIUS * 0.6);
  if (wire) return <Lines paths={sphereRings(center, radius)} radius={thickness} color={color} opacity={opacity} />;
  return (
    <mesh position={center}>
      <sphereGeometry args={[radius, 64, 48]} />
      <SolidMaterial color={colour} opacity={opacity} />
    </mesh>
  );
}

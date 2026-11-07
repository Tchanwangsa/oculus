import type { Colour } from "../colors";
import { themeColour } from "./colour";
import { gridLines, surfaceGrid } from "./geometry/surface";
import type { Vec3 } from "./geometry/vec";
import { Lines, SolidMaterial, useGeometry } from "./mesh";
import { LINE_RADIUS, useThickness } from "./scene";

/** Lines each way on a wire surface. */
const WIRE_LINES = 12;

export type SurfaceProps = {
  samples?: number;
  color?: Colour;
  opacity?: number;
  wire?: boolean;
} & (
  | { z: (x: number, y: number) => number; x: [number, number]; y: [number, number] }
  | { xyz: (u: number, v: number) => Vec3; u: [number, number]; v: [number, number] }
);

/**
 * A surface: the graph `z(x, y)` over `x` × `y`, or the parametric
 * `xyz(u, v)` over `u` × `v`, on a `samples` × `samples` grid. Lit on both
 * sides; with `wire`, its grid lines instead. A sample that isn't finite
 * leaves a hole.
 */
export function Surface(props: SurfaceProps) {
  const { samples = 48, color = "chart-1", opacity = 1, wire = false } = props;
  const colour = themeColour(color, "chart-1");
  const thickness = useThickness(LINE_RADIUS * 0.6);
  const grid =
    "z" in props
      ? surfaceGrid((x, y) => [x, y, props.z(x, y)], props.x, props.y, samples)
      : surfaceGrid(props.xyz, props.u, props.v, samples);
  const geometry = useGeometry(() => {
    if (wire) return null;
    return { positions: grid.positions, normals: new Float32Array(), indices: grid.indices };
  }, [grid, wire]);
  if (wire) return <Lines paths={gridLines(grid, samples, WIRE_LINES)} radius={thickness} color={color} opacity={opacity} />;
  if (!geometry) return null;
  return (
    <mesh geometry={geometry}>
      <SolidMaterial color={colour} opacity={opacity} doubleSided />
    </mesh>
  );
}

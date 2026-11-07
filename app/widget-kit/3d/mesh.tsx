import { useEffect, useMemo } from "react";
import { BufferAttribute, BufferGeometry, DoubleSide, FrontSide, type Color } from "three";
import type { Colour } from "../colors";
import { themeColour } from "./colour";
import { merge, tube, type Mesh } from "./geometry/tube";
import type { Vec3 } from "./geometry/vec";

/** What `build` returns as a geometry (normals computed when it has none),
 *  rebuilt when `deps` change and released when it is replaced or the part
 *  unmounts. */
export function useGeometry(build: () => Mesh | null, deps: unknown[]): BufferGeometry | null {
  const geometry = useMemo(() => toGeometry(build()), deps);
  useEffect(() => () => geometry?.dispose(), [geometry]);
  return geometry;
}

function toGeometry(mesh: Mesh | null): BufferGeometry | null {
  if (!mesh || !mesh.indices.length) return null;
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new BufferAttribute(mesh.positions, 3));
  geometry.setIndex(new BufferAttribute(mesh.indices, 1));
  if (mesh.normals.length) geometry.setAttribute("normal", new BufferAttribute(mesh.normals, 3));
  else geometry.computeVertexNormals();
  return geometry;
}

/** Lines and arrows: the token's colour exactly, unlit, as `oculus/2d` draws them. */
export function FlatMaterial({ color, opacity = 1 }: { color: Color; opacity?: number }) {
  const transparent = opacity < 1;
  return <meshBasicMaterial color={color} transparent={transparent} opacity={opacity} depthWrite={!transparent} toneMapped={false} />;
}

/** Solids: lit by the scene's lights; below full opacity, what is inside shows. */
export function SolidMaterial({ color, opacity = 1, doubleSided }: { color: Color; opacity?: number; doubleSided?: boolean }) {
  const transparent = opacity < 1;
  return (
    <meshStandardMaterial
      color={color}
      roughness={0.8}
      metalness={0}
      transparent={transparent}
      opacity={opacity}
      depthWrite={!transparent}
      side={doubleSided ? DoubleSide : FrontSide}
    />
  );
}

/** Polylines drawn as tubes of `radius`, all in one mesh. */
export function Lines({ paths, radius, color, opacity }: { paths: Vec3[][]; radius: number; color: Colour; opacity?: number }) {
  const colour = themeColour(color, "foreground");
  const geometry = useGeometry(() => merge(paths.map((p) => tube(p, radius))), [paths, radius]);
  if (!geometry) return null;
  return (
    <mesh geometry={geometry}>
      <FlatMaterial color={colour} opacity={opacity} />
    </mesh>
  );
}

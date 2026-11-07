import { createContext, useContext } from "react";
import { length, sub, type Vec3 } from "./geometry/vec";

/** Where `Scene3D`'s camera starts: above the first octant, z up. */
export const DEFAULT_CAMERA: Vec3 = [3.4, 2.6, 2.2];
export const DEFAULT_DISTANCE = length(DEFAULT_CAMERA);

/** Tube radius of a line at `weight` 1, in world units at the default view:
 *  about 2 CSS px across in a 320 px scene. */
export const LINE_RADIUS = 0.012;
/** An arrow's shaft radius and longest head at `weight` 1. */
export const ARROW_RADIUS = 0.018;
export const ARROW_HEAD = 0.2;

/** How far the starting camera is from its target against the default, so
 *  a scene framed from further away draws its lines as thick on screen. */
export const SceneScale = createContext(1);

export const sceneScale = (camera: Vec3, target: Vec3) => length(sub(camera, target)) / DEFAULT_DISTANCE || 1;

/** A thickness at `weight` (default 1) in this scene's world units. */
export function useThickness(base: number, weight?: number): number {
  return base * useContext(SceneScale) * (weight ?? 1);
}

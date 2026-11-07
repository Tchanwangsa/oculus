import { Circle as MafsCircle, Polygon as MafsPolygon } from "mafs";
import { paint } from "../colors";
import type { Stroke } from "./lines";

type Vec = [number, number];

export type Fill = Stroke & {
  /** 0 for an outline only. */
  fillOpacity?: number;
};

const filled = ({ color, weight, style, opacity, fillOpacity }: Fill) => ({
  color: paint(color, "chart-1"),
  weight,
  strokeStyle: style,
  strokeOpacity: opacity,
  fillOpacity,
});

/** A circle of `radius` (maths units) about `center`. */
export function Circle({ center, radius, ...rest }: Fill & { center: Vec; radius: number }) {
  return <MafsCircle center={center} radius={radius} {...filled(rest)} />;
}

/** A closed polygon through `points`, in order. */
export function Polygon({ points, ...rest }: Fill & { points: Vec[] }) {
  return <MafsPolygon points={points} {...filled(rest)} />;
}

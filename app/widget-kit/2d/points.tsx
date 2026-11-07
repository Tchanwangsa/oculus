import { useCallback } from "react";
import { MovablePoint as MafsMovablePoint, Point as MafsPoint } from "mafs";
import { paint, type Colour } from "../colors";

type Vec = [number, number];

/** A dot at `(x, y)`. */
export function Point({ x, y, color, opacity }: { x: number; y: number; color?: Colour; opacity?: number }) {
  return <MafsPoint x={x} y={y} color={paint(color, "foreground")} opacity={opacity} />;
}

export type Constraint = "horizontal" | "vertical" | ((point: Vec) => Vec);

/**
 * A point the student drags (or moves with the arrow keys once focused).
 * Controlled: draw it at `point` and store what `onMove` passes. `constrain`
 * keeps it on a line (`"horizontal"`, `"vertical"`) or maps where the pointer
 * is to where the point may be (snap to a curve, a grid, a circle).
 */
export function MovablePoint({
  point,
  onMove,
  constrain,
  color,
}: {
  point: Vec;
  onMove: (point: Vec) => void;
  constrain?: Constraint;
  color?: Colour;
}) {
  const [px, py] = point;
  const map = useCallback(
    ([x, y]: Vec): Vec => {
      if (constrain === "horizontal") return [x, py];
      if (constrain === "vertical") return [px, y];
      return constrain ? constrain([x, y]) : [x, y];
    },
    [constrain, px, py],
  );
  return (
    <MafsMovablePoint
      point={point}
      onMove={(p) => onMove([p[0], p[1]])}
      constrain={map}
      color={paint(color, "brand")}
    />
  );
}

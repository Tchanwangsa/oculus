import { useCallback } from "react";
import { Plot as MafsPlot } from "mafs";
import { paint } from "../colors";
import type { Stroke } from "./lines";

type Vec = [number, number];

export type PlotProps = Stroke &
  (
    | { y: (x: number) => number; domain?: Vec }
    | { x: (y: number) => number; domain?: Vec }
    | { xy: (t: number) => Vec; domain: Vec }
    | { r: (theta: number) => number; domain?: Vec }
  );

const TAU = 2 * Math.PI;

/** Polar `r(θ)` as the parametric curve it draws. */
function Polar({ r, domain = [0, TAU], ...stroke }: Omit<Stroke, "color"> & { color: string; r: (theta: number) => number; domain?: Vec }) {
  const xy = useCallback((t: number): Vec => {
    const radius = r(t);
    return [radius * Math.cos(t), radius * Math.sin(t)];
  }, [r]);
  return <MafsPlot.Parametric xy={xy} domain={domain} {...stroke} />;
}

/**
 * A curve: `y={(x) => …}` (a function of x across the plane, or over
 * `domain`), `x={(y) => …}`, `xy={(t) => [x, y]}` over `domain`, or polar
 * `r={(θ) => …}` over `domain` (default one turn). A value that isn't finite
 * is skipped.
 */
export function Plot(props: PlotProps) {
  const { color, weight, style, opacity } = props;
  const stroke = { color: paint(color, "chart-1"), weight, style, opacity };
  if ("y" in props) return <MafsPlot.OfX y={props.y} domain={props.domain} {...stroke} />;
  if ("x" in props) return <MafsPlot.OfY x={props.x} domain={props.domain} {...stroke} />;
  if ("xy" in props) return <MafsPlot.Parametric xy={props.xy} domain={props.domain} {...stroke} />;
  return <Polar r={props.r} domain={props.domain} {...stroke} />;
}

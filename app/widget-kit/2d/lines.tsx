import { Line as MafsLine, Vector as MafsVector } from "mafs";
import { paint, type Colour } from "../colors";

type Vec = [number, number];

export interface Stroke {
  color?: Colour;
  /** Stroke width in CSS px. */
  weight?: number;
  style?: "solid" | "dashed";
  opacity?: number;
}

const stroke = ({ color, weight, style, opacity }: Stroke, fallback: Colour) => ({
  color: paint(color, fallback),
  weight,
  style,
  opacity,
});

export type LineProps = Stroke & ({ point1: Vec; point2: Vec } | { point: Vec; slope: number } | { point: Vec; angle: number });

/** An infinite line through two points, or through a point with a slope or
 *  an angle (radians from the x-axis). */
export function Line(props: LineProps) {
  const s = stroke(props, "foreground");
  if ("point1" in props) return <MafsLine.ThroughPoints point1={props.point1} point2={props.point2} {...s} />;
  if ("slope" in props) return <MafsLine.PointSlope point={props.point} slope={props.slope} {...s} />;
  return <MafsLine.PointAngle point={props.point} angle={props.angle} {...s} />;
}

/** The segment between two points. */
export function Segment({ point1, point2, ...rest }: Stroke & { point1: Vec; point2: Vec }) {
  return <MafsLine.Segment point1={point1} point2={point2} {...stroke(rest, "foreground")} />;
}

/** An arrow from `tail` (default the origin) to `tip`. */
export function Vector({ tail, tip, ...rest }: Stroke & { tail?: Vec; tip: Vec }) {
  return <MafsVector tail={tail} tip={tip} {...stroke(rest, "foreground")} />;
}

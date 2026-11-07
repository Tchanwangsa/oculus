import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Coordinates, labelPi, Mafs } from "mafs";

type Range = [number, number];

export interface PlaneProps {
  children?: ReactNode;
  x?: Range;
  y?: Range;
  height?: number;
  grid?: "cartesian" | "polar" | "none";
  step?: number | Range;
  subdivisions?: number;
  labels?: boolean | "pi";
  axes?: boolean;
  aspect?: "equal" | "fill";
  pan?: boolean;
  onClick?: (point: [number, number]) => void;
}

export const DEFAULT_HEIGHT = 320;

/** A tick label with a true minus sign, centred on its tick by a hidden
 *  sign after it as Mafs does. */
function signed(text: string): ReactNode {
  if (!text.startsWith("-")) return text;
  return (
    <>
      {`\u2212${text.slice(1)}`}
      <tspan visibility="hidden">{"\u2212"}</tspan>
    </>
  );
}

/** A number tick, without float noise (0.30000000000000004). */
const tick = (v: number) => signed(String(Number(v.toPrecision(10))));
const piTick = (v: number) => signed(labelPi(v));
const pair = (step: number | Range): Range => (typeof step === "number" ? [step, step] : step);

/** The plane at a known width: what `Plane` draws once it has measured.
 *  Grid "none" keeps the lines' spacing for the labels; `Plane`'s class
 *  hides the lines themselves (kit.css). */
export function PlaneView({
  width,
  children,
  x = [-5, 5],
  y = [-5, 5],
  height = DEFAULT_HEIGHT,
  grid = "cartesian",
  step,
  subdivisions,
  labels = true,
  axes = true,
  aspect = "equal",
  pan = false,
  onClick,
}: PlaneProps & { width: number }) {
  const [xStep, yStep] = pair(step ?? (labels === "pi" ? [Math.PI / 2, 1] : 1));
  const xLabel = !axes || !labels ? false : labels === "pi" ? piTick : tick;
  const yLabel = !axes || !labels ? false : tick;
  const span = Math.max(x[1] - x[0], y[1] - y[0]);
  return (
    <Mafs
      width={width}
      height={height}
      pan={pan}
      zoom={false}
      viewBox={{ x, y, padding: span * 0.04 }}
      preserveAspectRatio={aspect === "equal" ? "contain" : false}
      onClick={onClick && ((point) => onClick([point[0], point[1]]))}
    >
      {grid === "polar" ? (
        <Coordinates.Polar
          lines={xStep}
          subdivisions={subdivisions}
          xAxis={{ axis: axes, lines: xStep, labels: xLabel }}
          yAxis={{ axis: axes, lines: yStep, labels: yLabel }}
        />
      ) : (
        <Coordinates.Cartesian
          subdivisions={grid === "none" ? false : (subdivisions ?? false)}
          xAxis={{ axis: axes, lines: xStep, labels: xLabel }}
          yAxis={{ axis: axes, lines: yStep, labels: yLabel }}
        />
      )}
      {children}
    </Mafs>
  );
}

/**
 * An SVG plane in maths coordinates, the full width of its column and
 * `height` CSS px tall from the first frame, so the widget never resizes when
 * it measures. Wheel and pinch never zoom it (the chat's scroll is the only
 * scroll) and it pans only with `pan`.
 */
export function Plane(props: PlaneProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    // The column's own CSS px, whatever transform is around the frame.
    const measure = () => setWidth(el.clientWidth);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  const height = props.height ?? DEFAULT_HEIGHT;
  return (
    <div ref={ref} className={props.grid === "none" ? "o-plane o-plane-bare" : "o-plane"} style={{ height }}>
      {width > 0 && <PlaneView {...props} width={width} />}
    </div>
  );
}

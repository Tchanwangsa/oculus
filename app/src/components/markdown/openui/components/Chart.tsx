import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { formatNumber } from "./format";

const W = 320;
const H = 160;
const PAD = { l: 44, r: 8, t: 8, b: 26 };

type Point = { x: number; y: number };

const points = (raw: unknown): Point[] =>
  Array.isArray(raw)
    ? raw.flatMap((p) => {
        const x = Number(p?.x);
        const y = Number(p?.y);
        return p && p.y !== null && Number.isFinite(x) && Number.isFinite(y) ? [{ x, y }] : [];
      })
    : [];

/** Three round ticks across [lo, hi], so the axis reads at a glance. */
function ticks(lo: number, hi: number): number[] {
  if (lo === hi) return [lo];
  return [lo, (lo + hi) / 2, hi];
}

/**
 * One series as an SVG line or bars, drawn by hand: the app ships no chart
 * library, and a study widget needs one curve, not a dashboard. Colours are
 * the chart tokens, so themes and dark mode follow.
 */
export const Chart = defineComponent({
  name: "Chart",
  description:
    "A line (default) or bar chart of {x, y} points, usually computed from sliders with @Each(@Range(...), \"t\", {x: t, y: ...}).",
  props: z.object({
    points: z.array(z.object({ x: z.number(), y: z.number().nullish() })),
    xLabel: z.string().optional(),
    yLabel: z.string().optional(),
    kind: z.enum(["line", "bar"]).optional(),
  }),
  component: function ChartView({ props }) {
    const data = points(props.points);
    if (data.length < 2) return <p className="my-2 text-xs text-muted-foreground">Not enough points to draw.</p>;
    const xs = data.map((p) => p.x);
    const ys = data.map((p) => p.y);
    const x0 = Math.min(...xs);
    const x1 = Math.max(...xs);
    const y0 = Math.min(0, ...ys);
    const y1 = Math.max(0, ...ys, y0 + 1e-9);
    const sx = (x: number) => PAD.l + ((x - x0) / (x1 - x0 || 1)) * (W - PAD.l - PAD.r);
    const sy = (y: number) => H - PAD.b - ((y - y0) / (y1 - y0 || 1)) * (H - PAD.t - PAD.b);
    const bar = props.kind === "bar";
    const bw = Math.max(2, ((W - PAD.l - PAD.r) / data.length) * 0.7);
    return (
      <figure className="my-2">
        <svg viewBox={`0 0 ${W} ${H}`} className="w-full overflow-visible" role="img" aria-label={props.yLabel ?? "Chart"}>
          {ticks(y0, y1).map((t) => (
            <g key={t}>
              <line x1={PAD.l} x2={W - PAD.r} y1={sy(t)} y2={sy(t)} className="stroke-border-subtle" />
              <text x={PAD.l - 6} y={sy(t)} dy="0.32em" textAnchor="end" className="fill-muted-foreground text-[9px] tabular-nums">
                {formatNumber(t, undefined, Math.abs(y1 - y0) < 10 ? 2 : 0)}
              </text>
            </g>
          ))}
          {ticks(x0, x1).map((t) => (
            <text key={t} x={sx(t)} y={H - PAD.b + 14} textAnchor="middle" className="fill-muted-foreground text-[9px] tabular-nums">
              {formatNumber(t, undefined, Math.abs(x1 - x0) < 10 ? 1 : 0)}
            </text>
          ))}
          {bar ? (
            data.map((p, i) => (
              <rect
                key={i}
                x={sx(p.x) - bw / 2}
                y={Math.min(sy(p.y), sy(0))}
                width={bw}
                height={Math.abs(sy(0) - sy(p.y))}
                rx={1.5}
                className="fill-chart-1"
              />
            ))
          ) : (
            <polyline
              points={data.map((p) => `${sx(p.x)},${sy(p.y)}`).join(" ")}
              fill="none"
              strokeWidth={2}
              strokeLinejoin="round"
              className="stroke-chart-1"
            />
          )}
        </svg>
        {(props.xLabel || props.yLabel) && (
          <figcaption className="mt-0.5 flex justify-between text-[11px] text-muted-foreground">
            <span>{props.yLabel ? `↑ ${props.yLabel}` : ""}</span>
            <span>{props.xLabel ? `${props.xLabel} →` : ""}</span>
          </figcaption>
        )}
      </figure>
    );
  },
});

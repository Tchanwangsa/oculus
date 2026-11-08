import { useState } from "react";
import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { area, densityMean, finite, points, type Point } from "./data";
import { ChartEmpty, ChartFigure, DataTable, Dot, HitBands, Marker, Plot, axisTicks } from "./Figure";
import { fraction, niceAxis, valueLabel } from "./scale";
import { point } from "./Series";

export type Region = { from: number; to: number };

/** Merge overlapping ranges so shaded probability never double-counts. */
export function regions(raw: unknown): Region[] {
  const sorted = Array.isArray(raw) ? raw.flatMap((r) => finite(r?.from) && finite(r?.to) && r.to > r.from ? [{ from: r.from, to: r.to }] : []).sort((a, b) => a.from - b.from) : [];
  const merged: Region[] = [];
  for (const r of sorted) {
    const last = merged[merged.length - 1];
    if (last && r.from <= last.to) last.to = Math.max(last.to, r.to);
    else merged.push({ ...r });
  }
  return merged;
}

/** The curve's points between two x values, with interpolated ends, so a
 *  shaded region starts and stops exactly at its bounds. */
function clip(pts: Point[], from: number, to: number): Point[] {
  const at = (x: number) => {
    const i = pts.findIndex((p) => p.x >= x);
    if (i <= 0) return pts[Math.max(0, i)].y;
    const a = pts[i - 1];
    const b = pts[i];
    return a.y + ((b.y - a.y) * (x - a.x)) / (b.x - a.x || 1);
  };
  const lo = Math.max(from, pts[0].x);
  const hi = Math.min(to, pts[pts.length - 1].x);
  if (hi <= lo) return [];
  return [{ x: lo, y: at(lo) }, ...pts.filter((p) => p.x > lo && p.x < hi), { x: hi, y: at(hi) }];
}

/**
 * A density curve with shaded regions, for "how likely is a value in this
 * range". With no hover, the readout gives the shaded share of the whole
 * area, so the picture and the probability arrive together.
 */
export const Distribution = defineComponent({
  name: "Distribution",
  description: "A density curve of {x, y} points with shaded {from, to} regions; the shaded share of the area is shown. mean marks the curve's mean.",
  props: z.object({
    title: z.string(),
    points: z.array(point),
    shade: z.array(z.object({ from: z.number(), to: z.number() })).optional(),
    xLabel: z.string().optional(),
    mean: z.boolean().optional(),
  }),
  component: function DistributionView({ props }) {
    const [active, setActive] = useState<number | null>(null);
    const pts = points(props.points).filter((p) => p.y >= 0).sort((a, b) => a.x - b.x);
    if (pts.length < 2) return <ChartFigure title={props.title}><ChartEmpty /></ChartFigure>;
    const lo = pts[0].x;
    const hi = pts[pts.length - 1].x;
    const xAxis = { ...niceAxis(lo, hi, 5), lo, hi };
    const yAxis = niceAxis(0, Math.max(...pts.map((p) => p.y)), 3, true);
    const sx = (x: number) => fraction(xAxis, x) * 100;
    const sy = (y: number) => 100 - fraction(yAxis, y) * 100;
    const poly = (part: Point[]) => `${sx(part[0].x)},${sy(0)} ${part.map((p) => `${sx(p.x)},${sy(p.y)}`).join(" ")} ${sx(part[part.length - 1].x)},${sy(0)}`;
    const total = area(pts);
    const shaded = regions(props.shade);
    const share = total > 0 && shaded.length ? shaded.reduce((s, r) => s + area(pts, r.from, r.to), 0) / total : null;
    const meanAt = densityMean(pts);
    const hovered = active !== null ? pts[active] : null;
    return <ChartFigure title={props.title}>
      <Plot yTicks={axisTicks(yAxis)} xTicks={axisTicks(xAxis)} xLabel={props.xLabel} yLabel="Density" onLeave={() => setActive(null)}
        readout={hovered ? <>{valueLabel(hovered.x)}<span className="ml-2 text-muted-foreground">density </span>{valueLabel(hovered.y)}</>
          : share !== null && <><span className="text-muted-foreground">shaded </span>{valueLabel(share * 100, "%")}</>}>
        <svg aria-hidden="true" viewBox="0 0 100 100" preserveAspectRatio="none" className="absolute inset-0 size-full overflow-visible">
          <polygon className="openui-mark fill-chart-1 opacity-10" points={poly(pts)} />
          {shaded.map((r, i) => { const part = clip(pts, r.from, r.to); return part.length > 1 && <polygon key={i} className="openui-mark fill-chart-1 opacity-35" points={poly(part)} />; })}
          <polyline fill="none" strokeWidth={2} strokeLinejoin="round" vectorEffect="non-scaling-stroke" className="openui-mark stroke-chart-1"
            points={pts.map((p) => `${sx(p.x)},${sy(p.y)}`).join(" ")} />
        </svg>
        {props.mean === true && finite(meanAt) && <Marker fx={fraction(xAxis, meanAt)} label="mean" />}
        {hovered && <Dot fx={sx(hovered.x) / 100} fy={fraction(yAxis, hovered.y)} fill="bg-chart-1" />}
        <HitBands at={pts.map((p) => sx(p.x) / 100)} onHover={setActive} />
      </Plot>
      <DataTable head={["x", "density"]} rows={pts.filter((_, i) => i % Math.ceil(pts.length / 40) === 0).map((p) => [valueLabel(p.x), valueLabel(p.y)])} />
    </ChartFigure>;
  },
});

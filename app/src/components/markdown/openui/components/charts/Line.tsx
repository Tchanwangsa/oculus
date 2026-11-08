import { useState } from "react";
import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { cn } from "@/lib/utils";
import { SERIES_FILL, SERIES_STROKE, SERIES_SVG_FILL, seriesList, type SeriesData } from "./data";
import { ChartEmpty, ChartFigure, DataTable, Dot, HitBands, Legend, Plot, axisTicks } from "./Figure";
import { fraction, niceAxis, valueLabel } from "./scale";
import { chartData } from "./Series";

/** Up to this many hover bands; a 400-point curve reads out every fourth. */
const MAX_BANDS = 100;

/** Lines over a shared x axis, drawn in a stretched SVG whose strokes keep
 *  their 2px width at any size. `area` washes the series fills. */
export function LineChart({ title, series, xLabel, yLabel, area }: { title?: unknown; series: SeriesData[]; xLabel?: unknown; yLabel?: unknown; area?: boolean }) {
  const [active, setActive] = useState<number | null>(null);
  const drawn = series.filter((s) => s.points.length >= 2);
  if (!drawn.length) return <ChartFigure title={title}><ChartEmpty /></ChartFigure>;
  const all = drawn.flatMap((s) => s.points);
  const xs = all.map((p) => p.x);
  const ys = all.map((p) => p.y);
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const minY = Math.min(...ys);
  const maxY = Math.max(...ys);
  const nice = niceAxis(minX, maxX);
  const xAxis = { ...nice, lo: minX, hi: maxX === minX ? minX + 1 : maxX };
  const yAxis = niceAxis(minY, maxY, 4, area || (minY >= 0 && minY < maxY - minY));
  const fy = (y: number) => fraction(yAxis, y);
  const sx = (x: number) => fraction(xAxis, x) * 100;
  const sy = (y: number) => 100 - fy(y) * 100;
  const base = [...drawn[0].points].sort((a, b) => a.x - b.x);
  const stride = Math.ceil(base.length / MAX_BANDS);
  const bands = base.filter((_, i) => i % stride === 0);
  const hovered = active !== null ? bands[active] : null;
  const nearest = (s: SeriesData, x: number) => s.points.reduce((best, p) => (Math.abs(p.x - x) < Math.abs(best.x - x) ? p : best));
  const readout = hovered && <>
    {valueLabel(hovered.x)}
    {drawn.map((s, i) => <span key={i} className="ml-2">{drawn.length > 1 && <span className="text-muted-foreground">{s.label} </span>}{valueLabel(nearest(s, hovered.x).y)}</span>)}
  </>;
  return <ChartFigure title={title}>
    <Plot yTicks={axisTicks(yAxis)} xTicks={axisTicks(xAxis)} xLabel={xLabel} yLabel={yLabel} readout={readout} onLeave={() => setActive(null)}>
      <svg aria-hidden="true" viewBox="0 0 100 100" preserveAspectRatio="none" className="absolute inset-0 size-full overflow-visible">
        {area && drawn.map((s, i) => {
          const pts = [...s.points].sort((a, b) => a.x - b.x);
          return <polygon key={i} className={cn("openui-mark opacity-15", SERIES_SVG_FILL[i])}
            points={`${sx(pts[0].x)},${sy(0)} ${pts.map((p) => `${sx(p.x)},${sy(p.y)}`).join(" ")} ${sx(pts[pts.length - 1].x)},${sy(0)}`} />;
        })}
        {drawn.map((s, i) => <polyline key={i} fill="none" strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke"
          className={cn("openui-mark", SERIES_STROKE[i])} points={[...s.points].sort((a, b) => a.x - b.x).map((p) => `${sx(p.x)},${sy(p.y)}`).join(" ")} />)}
      </svg>
      {hovered && <>
        <div aria-hidden="true" className="pointer-events-none absolute inset-y-0 w-px bg-border" style={{ left: `${sx(hovered.x)}%` }} />
        {drawn.map((s, i) => { const p = nearest(s, hovered.x); return <Dot key={i} fx={sx(p.x) / 100} fy={fy(p.y)} fill={SERIES_FILL[i]} />; })}
      </>}
      <HitBands at={bands.map((p) => sx(p.x) / 100)} onHover={setActive} />
    </Plot>
    <Legend entries={drawn.map((s, i) => ({ label: s.label, swatch: SERIES_FILL[i], line: true }))} />
    <DataTable head={["x", ...drawn.map((s) => s.label)]} rows={base.map((p) => [valueLabel(p.x), ...drawn.map((s) => valueLabel(nearest(s, p.x).y))])} />
  </ChartFigure>;
}

export const Line = defineComponent({
  name: "Line",
  description: "A line chart over x; several Series get a legend. area fills under the series.",
  props: z.object({
    title: z.string(),
    series: chartData,
    xLabel: z.string().optional(),
    yLabel: z.string().optional(),
    area: z.boolean().optional(),
  }),
  component: ({ props }) => <LineChart title={props.title} series={seriesList(props.series)} xLabel={props.xLabel} yLabel={props.yLabel} area={props.area === true} />,
});

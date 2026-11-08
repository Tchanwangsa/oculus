import { useState } from "react";
import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { cn } from "@/lib/utils";
import { binValues, binsFromPoints, finite, mean, numbers, points, type Bin } from "./data";
import { ChartEmpty, ChartFigure, DataTable, Marker, Plot, axisTicks } from "./Figure";
import { fraction, niceAxis, valueLabel } from "./scale";
import { point } from "./Series";

/**
 * Touching columns on a numeric x axis, split by the 2px surface gap.
 * `meanAt` draws the mean marker. Also draws the old `Chart(..., "bar")`,
 * whose points are column centres.
 */
export function BinChart({ title, bins, xLabel, yLabel, meanAt }: { title?: unknown; bins: Bin[]; xLabel?: unknown; yLabel?: unknown; meanAt?: number }) {
  const [active, setActive] = useState<number | null>(null);
  if (!bins.length) return <ChartFigure title={title}><ChartEmpty /></ChartFigure>;
  const lo = bins[0].x0;
  const hi = bins[bins.length - 1].x1;
  const xAxis = { ...niceAxis(lo, hi, 5), lo, hi };
  const counts = bins.map((b) => b.count);
  const yAxis = niceAxis(Math.min(0, ...counts), Math.max(...counts), 4, true);
  const zero = fraction(yAxis, 0);
  const hovered = active !== null ? bins[active] : null;
  return <ChartFigure title={title}>
    <Plot yTicks={axisTicks(yAxis)} xTicks={axisTicks(xAxis)} xLabel={xLabel} yLabel={yLabel} onLeave={() => setActive(null)}
      readout={hovered ? <>{valueLabel(hovered.x0)}–{valueLabel(hovered.x1)}<span className="ml-2">{valueLabel(hovered.count)}</span></>
        : meanAt !== undefined && <><span className="text-muted-foreground">mean </span>{valueLabel(meanAt)}</>}>
      <svg role="img" aria-label="Histogram bars" viewBox="0 0 320 160" preserveAspectRatio="none" className="absolute inset-0 size-full">
        {bins.map((b, i) => {
          const left = fraction(xAxis, b.x0);
          const f = fraction(yAxis, b.count);
          return <rect key={i} x={left * 320 + 0.5} y={(1 - Math.max(f, zero)) * 160}
            width={Math.max(0, (fraction(xAxis, b.x1) - left) * 320 - 1)} height={Math.abs(f - zero) * 160}
            className={cn("openui-mark fill-chart-1 transition-opacity", active !== null && active !== i && "opacity-50")}
            onPointerEnter={() => setActive(i)} onPointerDown={() => setActive(i)} />;
        })}
      </svg>
      {meanAt !== undefined && <Marker fx={fraction(xAxis, meanAt)} label="mean" />}
    </Plot>
    <DataTable head={["From", "To", "Count"]} rows={bins.map((b) => [valueLabel(b.x0), valueLabel(b.x1), valueLabel(b.count)])} />
  </ChartFigure>;
}

export const Histogram = defineComponent({
  name: "Histogram",
  description: "How values are spread: raw numbers are binned (bins defaults to √n), or pass {x, y} bin centres and counts. mean marks the average.",
  props: z.object({
    title: z.string(),
    values: z.array(z.union([z.number(), point])),
    bins: z.number().optional(),
    xLabel: z.string().optional(),
    mean: z.boolean().optional(),
  }),
  component: ({ props }) => {
    const raw = Array.isArray(props.values) ? props.values : [];
    const values = numbers(raw);
    const pts = points(raw.filter((v) => typeof v === "object"));
    const bins = values.length >= pts.length ? binValues(values, props.bins) : binsFromPoints(pts);
    const total = pts.reduce((s, p) => s + p.y, 0);
    const avg = values.length >= pts.length ? mean(values) : pts.reduce((s, p) => s + p.x * p.y, 0) / total;
    return <BinChart title={props.title} bins={bins} xLabel={props.xLabel} yLabel="Count"
      meanAt={props.mean === true && finite(avg) ? avg : undefined} />;
  },
});

import { useState } from "react";
import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { cn } from "@/lib/utils";
import { SERIES_FILL, SERIES_STROKE, fitLine, seriesList } from "./data";
import { ChartEmpty, ChartFigure, DataTable, Legend, Plot, axisTicks } from "./Figure";
import { fraction, niceAxis, valueLabel } from "./scale";
import { chartData } from "./Series";

/** Points as 8px dots with a surface ring; each dot's hit target is larger
 *  than the dot. `trend` adds each series' least-squares line. */
export const Scatter = defineComponent({
  name: "Scatter",
  description: "Points against two measures; several Series get a legend. trend draws each series' best-fit line.",
  props: z.object({
    title: z.string(),
    series: chartData,
    xLabel: z.string().optional(),
    yLabel: z.string().optional(),
    trend: z.boolean().optional(),
  }),
  component: function ScatterView({ props }) {
    const [active, setActive] = useState<[number, number] | null>(null);
    const series = seriesList(props.series).filter((s) => s.points.length);
    if (!series.length) return <ChartFigure title={props.title}><ChartEmpty /></ChartFigure>;
    const all = series.flatMap((s) => s.points);
    const xAxis = niceAxis(Math.min(...all.map((p) => p.x)), Math.max(...all.map((p) => p.x)));
    const yAxis = niceAxis(Math.min(...all.map((p) => p.y)), Math.max(...all.map((p) => p.y)));
    const sx = (x: number) => fraction(xAxis, x) * 100;
    const sy = (y: number) => 100 - fraction(yAxis, y) * 100;
    const hovered = active ? series[active[0]]?.points[active[1]] : null;
    return <ChartFigure title={props.title}>
      <Plot yTicks={axisTicks(yAxis)} xTicks={axisTicks(xAxis)} xLabel={props.xLabel} yLabel={props.yLabel} onLeave={() => setActive(null)}
        readout={hovered && <>{series.length > 1 && <span className="text-muted-foreground">{series[active![0]].label} </span>}({valueLabel(hovered.x)}, {valueLabel(hovered.y)})</>}>
        {props.trend === true && <svg aria-hidden="true" viewBox="0 0 100 100" preserveAspectRatio="none" className="absolute inset-0 size-full overflow-visible">
          {series.map((s, i) => {
            const fit = fitLine(s.points);
            if (!fit) return null;
            return <line key={i} x1={0} x2={100} y1={sy(fit.intercept + fit.slope * xAxis.lo)} y2={sy(fit.intercept + fit.slope * xAxis.hi)}
              strokeWidth={1.5} vectorEffect="non-scaling-stroke" className={cn("openui-mark opacity-60", SERIES_STROKE[i])} />;
          })}
        </svg>}
        <svg role="img" aria-label="Scatter points" viewBox="0 0 320 160" preserveAspectRatio="none" className="absolute inset-0 size-full overflow-hidden">
          {series.map((s, i) => s.points.map((p, j) => <circle key={`${i}-${j}`} cx={sx(p.x) * 3.2} cy={sy(p.y) * 1.6} r={3}
            className={cn("openui-mark stroke-card", SERIES_FILL[i].replace("bg-", "fill-"), active && (active[0] !== i || active[1] !== j) && "opacity-60")}
            onPointerEnter={() => setActive([i, j])} onPointerDown={() => setActive([i, j])} />))}
        </svg>
      </Plot>
      <Legend entries={series.map((s, i) => ({ label: s.label, swatch: SERIES_FILL[i] }))} />
      <DataTable head={["Series", "x", "y"]} rows={series.flatMap((s) => s.points.map((p) => [s.label, valueLabel(p.x), valueLabel(p.y)]))} />
    </ChartFigure>;
  },
});

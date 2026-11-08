import { useState } from "react";
import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { cn } from "@/lib/utils";
import { Rich } from "../Frame";
import { finite, strings } from "./data";
import { ChartEmpty, ChartFigure, DataTable, Plot, axisTicks } from "./Figure";
import { fraction, niceAxis, valueLabel } from "./scale";

/** Columns while the labels are short and few; past that, rows with the
 *  label beside each bar, so long names never collide under the axis. */
const COLUMN_MAX = 8;
const COLUMN_LABEL = 6;

export function BarChart({ title, labels, values, yLabel, unit }: { title?: unknown; labels: string[]; values: (number | null)[]; yLabel?: unknown; unit?: unknown }) {
  const [active, setActive] = useState<number | null>(null);
  const bars = values.flatMap((v, i) => (finite(v) ? [{ label: labels[i] ?? "", value: v }] : []));
  if (!bars.length) return <ChartFigure title={title}><ChartEmpty /></ChartFigure>;
  const axis = niceAxis(Math.min(...bars.map((b) => b.value)), Math.max(...bars.map((b) => b.value)), 4, true);
  const zero = fraction(axis, 0);
  const table = <DataTable head={["Label", typeof yLabel === "string" ? yLabel : "Value"]} rows={bars.map((b) => [b.label, valueLabel(b.value, unit)])} />;
  const columns = bars.length <= COLUMN_MAX && bars.every((b) => b.label.length <= COLUMN_LABEL);
  if (!columns) return <ChartFigure title={title}>
    {typeof yLabel === "string" && yLabel && <div className="mb-1 text-[11px] text-muted-foreground"><Rich text={yLabel} /></div>}
    <ul className="flex flex-col gap-1.5">
      {bars.map((b, i) => {
        const f = fraction(axis, b.value);
        return <li key={i} className="openui-mark grid grid-cols-[minmax(0,10rem)_minmax(0,1fr)] items-center gap-3 text-[12px]">
          <Rich text={b.label} className="truncate text-muted-foreground" />
          <div className="flex min-w-0 items-center gap-2">
            <svg aria-hidden="true" viewBox="0 0 100 12" preserveAspectRatio="none" className="h-3 min-w-0 flex-1">
              <rect x={Math.min(f, zero) * 100} y={0} width={Math.abs(f - zero) * 100} height={12} rx={1} className="fill-chart-1" />
            </svg>
            <span className="w-14 shrink-0 tabular-nums text-foreground">{valueLabel(b.value, unit)}</span>
          </div>
        </li>;
      })}
    </ul>
    {table}
  </ChartFigure>;
  const hovered = active !== null ? bars[active] : null;
  return <ChartFigure title={title}>
    <Plot yTicks={axisTicks(axis)} xTicks={bars.map((b, i) => ({ at: (i + 0.5) / bars.length, label: b.label }))} yLabel={yLabel} onLeave={() => setActive(null)}
      readout={hovered && <><span className="text-muted-foreground">{hovered.label} </span>{valueLabel(hovered.value, unit)}</>}
>
      <svg role="img" aria-label="Category bars" viewBox="0 0 320 160" preserveAspectRatio="none" className="absolute inset-0 size-full">
        {bars.map((b, i) => {
          const f = fraction(axis, b.value);
          const width = Math.min(24, 224 / bars.length);
          return <rect key={i} x={((i + 0.5) / bars.length) * 320 - width / 2} y={(1 - Math.max(f, zero)) * 160}
            width={width} height={Math.abs(f - zero) * 160}
            className={cn("openui-mark fill-chart-1 transition-opacity", active !== null && active !== i && "opacity-50")}
            onPointerEnter={() => setActive(i)} onPointerDown={() => setActive(i)} />;
        })}
      </svg>
    </Plot>
    {table}
  </ChartFigure>;
}

export const Bar = defineComponent({
  name: "Bar",
  description: "Bars comparing categories: labels[i] with values[i]; unit formats values, e.g. Bar(\"Hours this week\", [\"Mon\", \"Tue\"], [2, 3.5], null, \"h\").",
  props: z.object({
    title: z.string(),
    labels: z.array(z.string()),
    values: z.array(z.number().nullable()),
    yLabel: z.string().optional(),
    unit: z.string().optional(),
  }),
  component: ({ props }) => <BarChart title={props.title} labels={strings(props.labels)} values={Array.isArray(props.values) ? props.values : []} yLabel={props.yLabel} unit={props.unit} />,
});

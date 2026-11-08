import { useState } from "react";
import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { cn } from "@/lib/utils";
import { Rich } from "../Frame";
import { MAX_SERIES, SERIES_FILL, SERIES_STROKE, finite, strings } from "./data";
import { ChartEmpty, ChartFigure } from "./Figure";
import { valueLabel } from "./scale";

const R = 40;
const C = 2 * Math.PI * R;
/** The surface gap between segments, in the ring's own units (about 2px). */
const GAP = 1.6;

/**
 * Parts of one whole, at most five plus Other: the tail folds into the
 * reserved grey so no sixth hue is ever generated. The legend lists every
 * part with its value and share; hovering a part or its row lifts it.
 */
export const Donut = defineComponent({
  name: "Donut",
  description: "Parts of one whole: labels[i] with values[i], at most 5 parts (the rest fold into Other). center replaces the total in the middle.",
  props: z.object({
    title: z.string(),
    labels: z.array(z.string()),
    values: z.array(z.number()),
    center: z.string().optional(),
    unit: z.string().optional(),
  }),
  component: function DonutView({ props }) {
    const [active, setActive] = useState<number | null>(null);
    const labels = strings(props.labels);
    const raw = (Array.isArray(props.values) ? props.values : []).flatMap((v, i) => (finite(v) && v > 0 ? [{ label: labels[i] ?? "", value: v }] : []));
    if (!raw.length) return <ChartFigure title={props.title}><ChartEmpty /></ChartFigure>;
    const parts = raw.length > MAX_SERIES ? [...raw.slice(0, MAX_SERIES - 1), { label: "Other", value: raw.slice(MAX_SERIES - 1).reduce((s, p) => s + p.value, 0) }] : raw;
    const other = raw.length > MAX_SERIES ? parts.length - 1 : -1;
    const selected = active !== null && parts[active] ? active : null;
    const total = parts.reduce((s, p) => s + p.value, 0);
    let start = 0;
    return <ChartFigure title={props.title}>
      <div className="flex flex-wrap items-center gap-x-6 gap-y-3" onPointerLeave={() => setActive(null)}>
        <div className="relative size-36 tabular-nums shrink-0">
          <svg aria-hidden="true" viewBox="0 0 100 100" className="size-full -rotate-90">
            {parts.map((p, i) => {
              const len = (p.value / total) * C;
              const seg = <circle key={i} cx={50} cy={50} r={R} fill="none" strokeWidth={14}
                strokeDasharray={`${Math.max(0.01, len - (parts.length > 1 ? GAP : 0))} ${C}`} strokeDashoffset={-start}
                className={cn("openui-mark transition-opacity", i === other ? "stroke-chart-other" : SERIES_STROKE[i], active !== null && active !== i && "opacity-40")}
                onPointerEnter={() => setActive(i)} onPointerDown={() => setActive(i)} />;
              start += len;
              return seg;
            })}
          </svg>
          <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center">
            {selected !== null ? <>
              <span className="text-lg font-semibold text-foreground">{valueLabel((parts[selected!].value / total) * 100, "%")}</span>
              <Rich text={parts[selected!].label} className="max-w-24 truncate text-[11px] text-muted-foreground" />
            </> : <span className="max-w-24 text-lg font-semibold text-foreground">{typeof props.center === "string" ? <Rich text={props.center} /> : valueLabel(total, props.unit)}</span>}
          </div>
        </div>
        <ul className="flex min-w-0 flex-1 basis-40 flex-col gap-1 text-[13px]">
          {parts.map((p, i) => <li key={i} className={cn("flex items-center gap-2 rounded-md px-1.5 py-0.5", active === i && "bg-accent")}
            onPointerEnter={() => setActive(i)}>
            {parts.length > 1 && <span aria-hidden="true" className={cn("size-2 shrink-0 rounded-full", i === other ? "bg-chart-other" : SERIES_FILL[i])} />}
            <Rich text={p.label} className="min-w-0 flex-1 truncate text-foreground" />
            <span className="shrink-0 tabular-nums text-muted-foreground">{valueLabel(p.value, props.unit)} · {valueLabel((p.value / total) * 100, "%")}</span>
          </li>)}
        </ul>
      </div>
    </ChartFigure>;
  },
});

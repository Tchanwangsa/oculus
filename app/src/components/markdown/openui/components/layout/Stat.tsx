import { defineComponent, useIsStreaming } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { formatNumber } from "../format";
import { Rich } from "../Frame";

/** A headline figure: compact past 10,000 (12.9K), tabular digits at
 *  display size; strings pass through ("Week 9"). */
export function statText(value: unknown, unit: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value !== "number" || !Number.isFinite(value)) return "—";
  if (Math.abs(value) >= 1e4) {
    const [div, suffix] = Math.abs(value) >= 1e9 ? [1e9, "B"] : Math.abs(value) >= 1e6 ? [1e6, "M"] : [1e3, "K"];
    return formatNumber(Number((value / div).toFixed(1)), unit, 1).replace(/(\d)(?=\D*$)/, `$1${suffix}`);
  }
  return formatNumber(value, unit, 2);
}

export const Stat = defineComponent({
  name: "Stat",
  description: "One figure with its label, e.g. Stat(\"Due this week\", 4, null, \"2 overdue\"). Consecutive Stats share a row.",
  props: z.object({
    label: z.string(),
    value: z.union([z.number(), z.string()]).nullable(),
    unit: z.string().optional(),
    note: z.string().optional(),
  }),
  component: function StatView({ props }) {
    const streaming = useIsStreaming();
    const pending = streaming && props.value === undefined;
    return <div className="min-w-0 py-1">
      <Rich text={props.label} className="block truncate text-xs text-muted-foreground" />
      {pending ? <div aria-hidden="true" className="block-shimmer my-1 h-6 w-16 rounded" />
        : <div className="font-display tabular-nums text-2xl leading-tight font-semibold text-foreground">{statText(props.value, props.unit)}</div>}
      {typeof props.note === "string" && <Rich text={props.note} className="block text-xs text-muted-foreground" />}
    </div>;
  },
});

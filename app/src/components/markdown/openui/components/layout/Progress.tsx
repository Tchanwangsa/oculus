import { defineComponent, useIsStreaming } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Progress as Meter } from "@/components/ui/progress";
import { formatNumber } from "../format";
import { Rich } from "../Frame";

/** A meter towards a total: "3 / 5" by default, a percentage when max is
 *  100 or omitted. Full turns success green. */
export const Progress = defineComponent({
  name: "Progress",
  description: "A labelled bar towards max (default 100), e.g. Progress(\"Lectures watched\", 7, 12).",
  props: z.object({
    label: z.string(),
    value: z.number().nullable(),
    max: z.number().optional(),
    unit: z.string().optional(),
  }),
  component: function ProgressView({ props }) {
    const streaming = useIsStreaming();
    const max = typeof props.max === "number" && props.max > 0 ? props.max : 100;
    const value = typeof props.value === "number" && Number.isFinite(props.value) ? Math.min(max, Math.max(0, props.value)) : null;
    const pct = value === null ? 0 : (value / max) * 100;
    return <div className="py-1.5">
      <div className="mb-1 flex items-baseline justify-between gap-2 text-[13px]">
        <Rich text={props.label} className="min-w-0 truncate text-muted-foreground" />
        <span className="shrink-0 tabular-nums text-foreground">
          {value === null ? "—" : max === 100 && !props.unit ? `${formatNumber(value, undefined, 0)}%` : `${formatNumber(value, undefined, 1)} / ${formatNumber(max, props.unit, 1)}`}
        </span>
      </div>
      {streaming && value === null ? <div aria-hidden="true" className="block-shimmer h-2 rounded-full" />
        : <Meter value={pct} aria-label={typeof props.label === "string" ? props.label : undefined} indicatorClassName={pct >= 100 ? "bg-success" : undefined} />}
    </div>;
  },
});

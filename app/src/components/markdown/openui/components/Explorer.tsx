import { useId } from "react";
import { defineComponent, reactive, useStateField } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Slider as SliderBar } from "@/components/ui/slider";
import { Chart } from "./Chart";
import { formatNumber } from "./format";
import { Frame, Rich } from "./Frame";
import { Text } from "./Text";

const num = (v: unknown, fallback: number) => (typeof v === "number" && Number.isFinite(v) ? v : fallback);

/** A number the student drags; its first argument is the `$variable` every
 *  Value and Chart in the explorer reads. */
export const Slider = defineComponent({
  name: "Slider",
  description: "Drags a $variable between min and max; unit like \"%\", \"$\" or \"yr\" formats the readout.",
  props: z.object({
    value: reactive(z.number()),
    label: z.string(),
    min: z.number(),
    max: z.number(),
    step: z.number().optional(),
    unit: z.string().optional(),
  }),
  component: function SliderView({ props, statementId }) {
    const fallbackName = useId();
    const field = useStateField(props.label || statementId || fallbackName, props.value);
    const min = num(props.min, 0);
    const max = num(props.max, min + 100) > min ? num(props.max, min + 100) : min + 100;
    const step = num(props.step, 0) > 0 ? num(props.step, 1) : 1;
    const value = Math.min(max, Math.max(min, num(Number(field.value), min)));
    return (
      <div className="flex flex-col gap-1.5 py-1.5">
        <div className="flex items-baseline justify-between gap-2 text-[13px]">
          <Rich text={props.label} className="text-muted-foreground" />
          <span className="tabular-nums font-medium text-foreground">{formatNumber(value, props.unit, 4)}</span>
        </div>
        <SliderBar
          min={min}
          max={max}
          step={step}
          value={[value]}
          aria-label={props.label}
          onValueChange={([v]) => field.setValue(v)}
        />
      </div>
    );
  },
});

/** A computed readout: an expression over the sliders' `$variables`. */
export const Value = defineComponent({
  name: "Value",
  description: "A computed number shown large, e.g. Value(\"Balance\", $p * @Pow(1 + $r / 100, $n), \"$\", 2).",
  props: z.object({
    label: z.string(),
    value: z.number().nullable(),
    unit: z.string().optional(),
    decimals: z.number().optional(),
  }),
  component: ({ props }) => (
    <div className="flex items-baseline justify-between gap-2 border-t border-border-subtle py-2 first:border-t-0">
      <Rich text={props.label} className="text-[13px] text-muted-foreground" />
      <span className="text-lg font-semibold tabular-nums text-foreground">
        {formatNumber(props.value, props.unit, props.decimals)}
      </span>
    </div>
  ),
});

/** A formula explorer: sliders over `$variables`, with Values and a Chart
 *  computed from them, so the student sees what each input does. */
export const Explorer = defineComponent({
  name: "Explorer",
  description:
    "A card of Sliders bound to $variables, with Values and Charts computed from them, plus Text for the formula.",
  props: z.object({
    title: z.string(),
    items: z.array(z.union([Slider.ref, Value.ref, Chart.ref, Text.ref])),
  }),
  component: function ExplorerView({ props, renderNode }) {
    return (
      <Frame title={typeof props.title === "string" ? props.title : undefined}>
        <div className="flex flex-col">{renderNode(props.items)}</div>
      </Frame>
    );
  },
});

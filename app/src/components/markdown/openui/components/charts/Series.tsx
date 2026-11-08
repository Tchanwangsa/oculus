import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";

export const point = z.object({ x: z.number(), y: z.number().nullish() });

/** Data only: a chart reads its label and points. */
export const Series = defineComponent({
  name: "Series",
  description: "One named series of numbers or {x, y} points, e.g. Series(\"n = 30\", @Each(...)). Only inside Line or Scatter.",
  props: z.object({
    label: z.string(),
    points: z.array(z.union([z.number(), point])),
  }),
  component: () => null,
});

/** Plain data for one series, or named Series nodes for comparison. */
export const chartData = z.array(z.union([z.number(), point, Series.ref]));

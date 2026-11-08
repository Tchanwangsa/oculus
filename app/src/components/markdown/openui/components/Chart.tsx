import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { BinChart } from "./charts/Histogram";
import { LineChart } from "./charts/Line";
import { binsFromPoints, points } from "./charts/data";
import { point } from "./charts/Series";

/**
 * The compatibility signature for stored programs and saved artifacts: it renders through `Line` or, as bars, the histogram's columns.
 * `prompt.ts` hides it from the generated grammar; new programs use the
 * chart family.
 */
export const Chart = defineComponent({
  name: "Chart",
  description: "Old single-series chart; use Line or Bar.",
  props: z.object({
    points: z.array(point),
    xLabel: z.string().optional(),
    yLabel: z.string().optional(),
    kind: z.enum(["line", "bar"]).optional(),
  }),
  component: function ChartView({ props }) {
    const data = points(props.points);
    if (props.kind === "bar") return <BinChart bins={binsFromPoints(data)} xLabel={props.xLabel} yLabel={props.yLabel} />;
    return <LineChart series={[{ label: typeof props.yLabel === "string" ? props.yLabel : "y", points: data }]} xLabel={props.xLabel} yLabel={props.yLabel} />;
  },
});

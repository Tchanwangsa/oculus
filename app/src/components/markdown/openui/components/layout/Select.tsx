import { useId } from "react";
import { defineComponent, reactive, useIsStreaming, useStateField } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Select as Picker, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { MoreComing } from "../../Streaming";
import { Rich } from "../Frame";
import { strings } from "../charts/data";

/** A choice among a few strings, bound to a string $variable; charts and
 *  ternaries read it to switch what they show. */
export const Select = defineComponent({
  name: "Select",
  description: "A dropdown bound to a string $variable; the options are the values, e.g. Select($dist, \"Population\", [\"Uniform\", \"Skewed\"]).",
  props: z.object({
    value: reactive(z.string()),
    label: z.string(),
    options: z.array(z.string()),
  }),
  component: function SelectView({ props, statementId }) {
    const id = useId();
    const streaming = useIsStreaming();
    const field = useStateField(`select:${props.label || statementId || id}`, props.value);
    const options = strings(props.options).filter(Boolean);
    const value = typeof field.value === "string" && options.includes(field.value) ? field.value : undefined;
    if (streaming && typeof props.label !== "string") return <MoreComing />;
    return <div className="flex items-center justify-between gap-3 py-1.5 text-[13px]">
      <label htmlFor={id} className="min-w-0"><Rich text={props.label} className="text-muted-foreground" /></label>
      <Picker value={value} onValueChange={(v) => field.setValue(v)} disabled={streaming || !options.length}>
        <SelectTrigger id={id} size="sm" className="min-w-32 text-[13px]"><SelectValue placeholder="Choose" /></SelectTrigger>
        <SelectContent>{options.map((o) => <SelectItem key={o} value={o}>{o}</SelectItem>)}</SelectContent>
      </Picker>
    </div>;
  },
});

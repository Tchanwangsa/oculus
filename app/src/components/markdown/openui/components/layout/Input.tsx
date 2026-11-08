import { useId } from "react";
import { defineComponent, reactive, useIsStreaming, useStateField } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Input as Field } from "@/components/ui/input";
import { MoreComing } from "../../Streaming";
import { Rich } from "../Frame";

/** A typed answer bound to a $variable. A $variable declared as a number
 *  stays a number, so `$guess = 0` can be compared with a computed value. */
export const Input = defineComponent({
  name: "Input",
  description: "A text field bound to a $variable; a number default keeps it numeric, e.g. $guess = 0 then Input($guess, \"Your estimate\").",
  props: z.object({
    value: reactive(z.union([z.string(), z.number()])),
    label: z.string(),
    placeholder: z.string().optional(),
  }),
  component: function InputView({ props, statementId }) {
    const id = useId();
    const streaming = useIsStreaming();
    const field = useStateField(`input:${props.label || statementId || id}`, props.value);
    const numeric = typeof field.value === "number";
    if (streaming && typeof props.label !== "string") return <MoreComing />;
    return <div className="flex flex-col gap-1 py-1.5 text-[13px]">
      <label htmlFor={id}><Rich text={props.label} className="text-muted-foreground" /></label>
      <Field id={id} disabled={streaming} inputMode={numeric ? "decimal" : undefined} placeholder={typeof props.placeholder === "string" ? props.placeholder : undefined}
        value={field.value == null ? "" : String(field.value)}
        onChange={(e) => {
          const text = e.target.value;
          const n = Number(text);
          field.setValue(numeric ? (text.trim() && Number.isFinite(n) ? n : 0) : text);
        }} />
    </div>;
  },
});

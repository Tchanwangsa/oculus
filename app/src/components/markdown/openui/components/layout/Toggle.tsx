import { useId } from "react";
import { defineComponent, reactive, useIsStreaming, useStateField } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Switch } from "@/components/ui/switch";
import { MoreComing } from "../../Streaming";
import { Rich } from "../Frame";

/** An on/off switch bound to a boolean $variable. */
export const Toggle = defineComponent({
  name: "Toggle",
  description: "A switch bound to a boolean $variable, e.g. $done = false then Toggle($done, \"Read chapter 3\").",
  props: z.object({
    value: reactive(z.boolean()),
    label: z.string(),
  }),
  component: function ToggleView({ props, statementId }) {
    const id = useId();
    const streaming = useIsStreaming();
    const field = useStateField(`toggle:${props.label || statementId || id}`, props.value);
    if (streaming && typeof props.label !== "string") return <MoreComing />;
    return <label htmlFor={id} className="flex cursor-pointer items-center justify-between gap-3 py-1.5 text-[13px]">
      <Rich text={props.label} className="min-w-0 text-foreground" />
      <Switch id={id} checked={field.value === true} disabled={streaming} onCheckedChange={(v) => field.setValue(v)} />
    </label>;
  },
});

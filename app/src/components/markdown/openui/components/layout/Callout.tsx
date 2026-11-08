import { defineComponent, useIsStreaming } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Info, Lightbulb, Warning } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Rich } from "../Frame";

const TONES = {
  note: { Icon: Info, icon: "text-muted-foreground", box: "border-border bg-surface" },
  tip: { Icon: Lightbulb, icon: "text-brand", box: "border-brand/25 bg-brand-muted" },
  warning: { Icon: Warning, icon: "text-warning", box: "border-warning/25 bg-warning/10" },
};

/** A set-apart remark: a rule of thumb, a common mistake, a caveat. */
export const Callout = defineComponent({
  name: "Callout",
  description: "A boxed remark beside the content: tone note (default), tip or warning, with an optional bold title.",
  props: z.object({
    text: z.string(),
    tone: z.enum(["note", "tip", "warning"]).optional(),
    title: z.string().optional(),
  }),
  component: function CalloutView({ props }) {
    const streaming = useIsStreaming();
    const tone = TONES[props.tone ?? "note"] ?? TONES.note;
    return <div role="note" className={cn("my-2 flex gap-2.5 rounded-lg border px-3 py-2.5 text-sm", tone.box)}>
      <tone.Icon size={16} weight="duotone" className={cn("mt-0.5 shrink-0", tone.icon)} />
      <div className="min-w-0 flex-1 leading-relaxed">
        {typeof props.title === "string" && <Rich text={props.title} className="mr-1.5 font-semibold text-foreground" />}
        {typeof props.text === "string" ? <Rich text={props.text} className="text-foreground/90" />
          : streaming && <div aria-hidden="true" className="block-shimmer mt-1 h-3 w-2/3 rounded" />}
      </div>
    </div>;
  },
});

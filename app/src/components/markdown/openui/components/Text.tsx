import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Rich } from "./Frame";

/** A line of prose between widgets, or a formula inside an Explorer. */
export const Text = defineComponent({
  name: "Text",
  description: "A short line of markdown; $…$ for maths. Strings may be built with + from $variables.",
  props: z.object({
    text: z.string(),
    tone: z.enum(["normal", "muted"]).optional(),
  }),
  component: ({ props }) => (
    <p className="my-1.5 text-sm leading-relaxed">
      <Rich text={props.text} className={props.tone === "muted" ? "text-muted-foreground" : "text-foreground/90"} />
    </p>
  ),
});

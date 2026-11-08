import { action, defineComponent, useIsStreaming, useTriggerAction } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Pill } from "../Frame";

/** The steps a Button may run: ask the assistant, set a $variable,
 *  re-run a Query. Any other step refuses the action. */
const ALLOWED = new Set(["continue_conversation", "set", "run"]);

type Plan = { steps: { type: string }[] };
const isPlan = (a: unknown): a is Plan => !!a && typeof a === "object" && Array.isArray((a as Plan).steps);

export function allowedPlan(raw: unknown): Plan | undefined {
  if (!isPlan(raw) || !raw.steps.length || raw.steps.some((s) => !ALLOWED.has(s?.type))) return undefined;
  return raw;
}

/** A pill that runs its action; without one it sends its label to the
 *  assistant, like Ask. Disabled while the fence streams. */
export const Button = defineComponent({
  name: "Button",
  description: "A pill that runs @ToAssistant, @Set or @Run steps, e.g. Button(\"Resample\", @Set($seed, $seed + 1)). Without an action it sends its label.",
  props: z.object({
    label: z.string(),
    action: action().optional(),
    tone: z.enum(["primary", "outline"]).optional(),
  }),
  component: function ButtonView({ props }) {
    const trigger = useTriggerAction();
    const streaming = useIsStreaming();
    if (typeof props.label !== "string" || !props.label) return streaming ? <span aria-hidden="true" className="block-shimmer inline-block h-7 w-20 rounded-full" /> : null;
    const plan = allowedPlan(props.action);
    if (props.action && !plan && (props.action as { type?: string }).type !== "continue_conversation") return null;
    return <Pill tone={props.tone === "primary" ? "primary" : "outline"} disabled={streaming}
      onClick={() => void trigger(props.label, undefined, (plan ?? props.action) as never)}>{props.label}</Pill>;
  },
});

import { action, defineComponent, useTriggerAction } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { ChatCircle } from "@phosphor-icons/react";
import { useAssistantSend } from "../assistant";
import { Pill } from "./Frame";

/**
 * A follow-up the student can send with one click, e.g. "Quiz me on the ones
 * I missed". The click goes through the runtime's action plan so
 * `@ToAssistant(message)` works as the language documents; `OpenUIBlock`'s
 * `onAction` hands the message to the thread. Hidden where no thread listens.
 */
export const Ask = defineComponent({
  name: "Ask",
  description:
    "A button that sends a follow-up message as the student, e.g. Ask(\"Harder questions\", @ToAssistant(\"Give me five harder Big-O questions\")). Without an action it sends its label.",
  props: z.object({
    label: z.string(),
    action: action().optional(),
  }),
  component: function AskView({ props }) {
    const send = useAssistantSend();
    const trigger = useTriggerAction();
    if (!send || typeof props.label !== "string") return null;
    return (
      <Pill onClick={() => void trigger(props.label, undefined, props.action as never)}>
        <ChatCircle size={13} /> {props.label}
      </Pill>
    );
  },
});

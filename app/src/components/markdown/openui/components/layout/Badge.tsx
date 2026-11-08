import { defineComponent, useIsStreaming } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Badge as BadgePill } from "@/components/ui/badge";
import { MoreComing } from "../../Streaming";
import { Rich } from "../Frame";

const VARIANT = { neutral: "secondary", brand: "default", success: "success", warning: "warning", danger: "destructive" } as const;

/** A status or category pill; tone says what it means, its text says it too. */
export const Badge = defineComponent({
  name: "Badge",
  description: "A small labelled pill, e.g. Badge(\"Overdue\", \"danger\"). Consecutive Badges and Buttons share a row.",
  props: z.object({
    text: z.string(),
    tone: z.enum(["neutral", "brand", "success", "warning", "danger"]).optional(),
  }),
  component: ({ props }) => {
    const streaming = useIsStreaming();
    return typeof props.text === "string" && props.text
    ? <BadgePill variant={VARIANT[props.tone ?? "neutral"] ?? "secondary"}><Rich text={props.text} /></BadgePill>
    : streaming ? <MoreComing /> : null;
  },
});

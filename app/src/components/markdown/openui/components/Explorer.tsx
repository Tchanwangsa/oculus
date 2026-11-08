import { useContext } from "react";
import { defineComponent, useIsStreaming } from "@openuidev/react-lang";
import { MoreComing, StreamingPending } from "../Streaming";
import { z } from "zod/v4";
import { Chart } from "./Chart";
import { Frame } from "./Frame";
import { Block } from "./layout/Block";
import { InCard } from "./layout/inCard";

export { Slider, Value } from "./Slider";

/** A formula explorer: sliders over `$variables`, with values and charts
 *  computed from them, so the student sees what each input does. */
export const Explorer = defineComponent({
  name: "Explorer",
  description: "A card of Sliders bound to $variables, with Values and charts computed from them, plus Text for the formula.",
  props: z.object({
    title: z.string(),
    items: z.array(z.union([Chart.ref, Block])),
  }),
  component: function ExplorerView({ props, renderNode }) {
    const streaming = useIsStreaming();
    const pendingContent = useContext(StreamingPending);
    return (
      <Frame title={typeof props.title === "string" ? props.title : undefined}>
        <InCard.Provider value={true}>
        <div className="flex flex-col">{streaming ? (props.items ?? []).map((item, index) => {
          const fields = item?.props as Record<string, unknown> | undefined;
          const pending = !fields
            || (item.typeName === "Slider" && [fields.value, fields.label, fields.min, fields.max].some((value) => value == null))
            || (item.typeName === "Value" && fields.value === undefined);
          return pending ? <MoreComing key={index} /> : <div key={index}>{renderNode(item)}</div>;
        }) : renderNode(props.items)}{streaming && pendingContent && <MoreComing />}</div>
        </InCard.Provider>
      </Frame>
    );
  },
});

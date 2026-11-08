import { useContext } from "react";
import { defineComponent, useIsStreaming } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { MoreComing, StreamingStatus } from "../../Streaming";
import { Frame, Rich } from "../Frame";
import { Block } from "./Block";
import { InCard } from "./inCard";
import { Items, PendingItems } from "./items";

/** A titled card of blocks. Inside another card it is a titled section
 *  under a hairline instead, so cards never nest. */
export const Panel = defineComponent({
  name: "Panel",
  description: "A titled card of Blocks; \"\" for no title.",
  props: z.object({
    title: z.string(),
    items: z.array(Block),
  }),
  component: function PanelView({ props, renderNode }) {
    const inCard = useContext(InCard);
    const streaming = useIsStreaming();
    const title = typeof props.title === "string" && props.title ? props.title : undefined;
    const empty = !Array.isArray(props.items) || !props.items.length;
    const body = <InCard.Provider value={true}>
      <Items items={props.items} renderNode={renderNode} />
      {streaming && (empty ? <MoreComing /> : <PendingItems items={props.items} />)}
    </InCard.Provider>;
    if (!inCard) return <Frame title={title} footer={streaming ? <StreamingStatus /> : undefined}>{body}</Frame>;
    return <section className="mt-3 border-t border-border-subtle pt-2 first:mt-0 first:border-t-0 first:pt-0">
      {title && <Rich text={title} className="mb-1 block text-[13px] font-semibold text-foreground" />}
      {body}
    </section>;
  },
});

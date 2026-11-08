import { defineComponent, useIsStreaming } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { MoreComing } from "../../Streaming";
import { Rich } from "../Frame";

/** Short reading-order items without another card around each line. */
export const List = defineComponent({
  name: "List",
  description: "Short markdown items as bullets; ordered=true numbers the steps.",
  props: z.object({ items: z.array(z.string()), ordered: z.boolean().optional() }),
  component: function ListView({ props }) {
    const streaming = useIsStreaming();
    const items = Array.isArray(props.items) ? props.items : [];
    const Tag = props.ordered ? "ol" : "ul";
    return <><Tag className={`my-2 space-y-1 pl-5 text-sm ${props.ordered ? "list-decimal tabular-nums" : "list-disc"}`}>
      {items.map((item, i) => <li key={i} className="openui-mark"><Rich text={item} /></li>)}
    </Tag>{streaming && (!items.length || items.some((item) => typeof item !== "string")) && <MoreComing />}</>;
  },
});

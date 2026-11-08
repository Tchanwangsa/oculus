import { defineComponent, useIsStreaming } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { MoreComing } from "../../Streaming";
import { Block } from "./Block";
import { Items } from "./items";
import { Panel } from "./Panel";

/** Children side by side, each at least 14rem wide, wrapping below each
 *  other when the reply is narrow; measured by CSS grid, not by script. */
export const Columns = defineComponent({
  name: "Columns",
  description: "Panels or Blocks side by side; they stack when the reply is narrow.",
  props: z.object({
    items: z.array(z.union([Panel.ref, Block])),
  }),
  component: function ColumnsView({ props, renderNode }) {
    const streaming = useIsStreaming();
    const items = Array.isArray(props.items) ? props.items : [];
    return <div className="my-3 grid grid-cols-[repeat(auto-fit,minmax(min(14rem,100%),1fr))] gap-3">
      {items.map((item, i) => <div key={i} className="min-w-0 *:my-0">
        {item && typeof item === "object" && "typeName" in item ? <Items items={[item]} renderNode={renderNode} /> : streaming && <MoreComing />}
      </div>)}
      {streaming && !items.length && <MoreComing />}
    </div>;
  },
});

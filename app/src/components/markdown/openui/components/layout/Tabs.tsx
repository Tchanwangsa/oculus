import { useContext, useId } from "react";
import { defineComponent, useIsStreaming, useStateField } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { MoreComing } from "../../Streaming";
import { Frame } from "../Frame";
import { Block } from "./Block";
import { Columns } from "./Columns";
import { InCard } from "./inCard";
import { Items, PendingItems } from "./items";
import { Panel } from "./Panel";

/** Data only: Tabs draws the strip and the chosen tab's items. */
export const Tab = defineComponent({
  name: "Tab",
  description: "One tab's label and content. Only inside Tabs.",
  props: z.object({
    label: z.string(),
    items: z.array(z.union([Panel.ref, Columns.ref, Block])),
  }),
  component: () => null,
});

type TabNode = { props?: { label?: unknown; items?: unknown } } | null;

/** Alternative views of one thing, e.g. two scenarios side by side in time.
 *  The chosen tab is widget state, so it survives a remount. */
export const Tabs = defineComponent({
  name: "Tabs",
  description: "A card with a tab strip; one Tab shows at a time.",
  props: z.object({
    tabs: z.array(Tab.ref),
  }),
  component: function TabsView({ props, renderNode, statementId }) {
    const inCard = useContext(InCard);
    const streaming = useIsStreaming();
    const id = useId();
    const tabs = (Array.isArray(props.tabs) ? props.tabs : []).map((t) => (t as TabNode)?.props ?? {});
    const field = useStateField(`tabs:${statementId ?? id}`, 0);
    const at = Math.min(Math.max(0, Number(field.value) || 0), Math.max(0, tabs.length - 1));
    const current = tabs[at];
    const strip = <PillTabs tabs={tabs.map((t, i) => ({ value: String(i), label: typeof t.label === "string" ? t.label : "…" }))}
      value={String(at)} onChange={(v) => field.setValue(Number(v))} scroll />;
    const body = <InCard.Provider value={true}>
      <fieldset disabled={streaming} className="mb-2 -ml-2 min-w-0 overflow-hidden">{strip}</fieldset>
      {current?.items ? <><Items items={current.items} renderNode={renderNode} />{streaming && <PendingItems items={current.items} />}</> : streaming && <MoreComing />}
    </InCard.Provider>;
    return inCard ? <div className="my-2">{body}</div> : <Frame>{body}</Frame>;
  },
});

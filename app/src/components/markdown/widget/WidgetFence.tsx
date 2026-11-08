import type { ReactNode } from "react";
import { useWidgetRow } from "@/components/markdown/openui/state";
import { widgetKey } from "@/stores/chat/widgetStore";
import { LiveWidget } from "./LiveWidget";
import { useWidgetStream } from "./stream/context";
import { widgetTitle } from "./title";
import { Building, WidgetBar, WidgetCard } from "./WidgetBar";

/**
 * A ```widget fence (docs/widgets.md), reached from `MD_COMPONENTS.pre`.
 * In a committed row it runs. In the reply still streaming as the row it
 * will become (`WidgetStream`), it previews: the same `LiveWidget`, so the
 * row keeps its frame when the reply commits. Anywhere else — `LiveTail`,
 * a library file — it is only its card saying "Building <title>…": no
 * source, no frame. `code` is the fence drawn as code, for the Source toggle.
 */
export function WidgetFence({ source, blockIndex, code }: { source: string; blockIndex: number; code: ReactNode }) {
  const row = useWidgetRow();
  const stream = useWidgetStream();
  const title = widgetTitle(source);
  const md = `\`\`\`widget\n${source}\n\`\`\``;
  if (!row && !stream) {
    return (
      <WidgetCard md={md}>
        <WidgetBar title={title} />
        <Building title={title} />
      </WidgetCard>
    );
  }
  // While streaming nothing is saved, so the key only has to be this fence's.
  const storeKey = row ? widgetKey(row.thread, row.row, blockIndex) : `stream:${stream!.thread}:${stream!.run}:${blockIndex}`;
  return <LiveWidget source={source} title={title} md={md} storeKey={storeKey} code={code} streaming={!row} />;
}

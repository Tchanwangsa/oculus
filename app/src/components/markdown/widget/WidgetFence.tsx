import type { ReactNode } from "react";
import { useWidgetRow } from "@/components/markdown/openui/state";
import { widgetKey } from "@/stores/chat/widgetStore";
import { LiveWidget } from "./LiveWidget";
import { widgetTitle } from "./title";
import { Building, WidgetBar, WidgetCard } from "./WidgetBar";

/**
 * A ```widget fence (docs/widgets.md), reached from `MD_COMPONENTS.pre`.
 * Until the reply is a committed row — while it streams through `LiveTail` —
 * it is only its card saying "Building <title>…": no source, no frame, so
 * nothing loads twice when the row commits and remounts. `code` is the fence
 * drawn as code, for the Source toggle.
 */
export function WidgetFence({ source, blockIndex, code }: { source: string; blockIndex: number; code: ReactNode }) {
  const row = useWidgetRow();
  const title = widgetTitle(source);
  const md = `\`\`\`widget\n${source}\n\`\`\``;
  if (!row) {
    return (
      <WidgetCard md={md}>
        <WidgetBar title={title} />
        <Building title={title} />
      </WidgetCard>
    );
  }
  return <LiveWidget source={source} title={title} md={md} storeKey={widgetKey(row.thread, row.row, blockIndex)} code={code} />;
}

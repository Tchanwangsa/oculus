import { useIsStreaming } from "@openuidev/react-lang";
import { useContext, type ReactNode } from "react";
import { WidgetIndex } from "../../state";
import { MoreComing, StreamingStatus } from "../../Streaming";
import { Frame } from "../Frame";
import { InCard } from "./inCard";

type Node = { typeName?: string; props?: unknown } | null | undefined;
const typeOf = (n: unknown) => (n as Node)?.typeName;

/** Small figures that read best side by side. */
const STAT = new Set(["Stat"]);
/** Pills that wrap along one line. */
const CHIP = new Set(["Badge", "Button"]);
/** Controls and readouts with no card of their own: at the top level a run
 *  of them shares one card. Charts, Panels, Callouts and study widgets draw
 *  their own. */
const BARE = new Set(["Stat", "Table", "Progress", "Toggle", "Select", "Input", "Slider", "Value", "Badge", "Button"]);

type Run = { kind: "stat" | "chip" | "card" | "one"; items: { node: unknown; index: number }[] };

function runs(items: unknown[], grouped: boolean): Run[] {
  const out: Run[] = [];
  items.forEach((node, index) => {
    const t = typeOf(node) ?? "";
    const kind: Run["kind"] = grouped && BARE.has(t) ? "card" : STAT.has(t) ? "stat" : CHIP.has(t) ? "chip" : "one";
    const last = out[out.length - 1];
    if (last && last.kind === kind && kind !== "one") last.items.push({ node, index });
    else out.push({ kind, items: [{ node, index }] });
  });
  return out;
}

/**
 * A container's children in reading order: consecutive Stats share a grid
 * row, consecutive Badges and Buttons a wrapping pill row, and outside any
 * card a run of bare controls shares one card. Each child keeps its
 * position as `WidgetIndex`, the key its saved progress lives under.
 */
export function Items({ items, renderNode }: { items: unknown; renderNode: (n: unknown) => ReactNode }) {
  const inCard = useContext(InCard);
  const streaming = useIsStreaming();
  const list = Array.isArray(items) ? items : [];
  const one = ({ node, index }: { node: unknown; index: number }) =>
    <WidgetIndex.Provider key={index} value={index}>{node && typeof node === "object" ? renderNode(node) : null}</WidgetIndex.Provider>;
  const draw = (run: Run, key: number): ReactNode => {
    if (run.kind === "stat") return <div key={key} className="my-1 grid grid-cols-[repeat(auto-fit,minmax(8rem,1fr))] gap-x-4 gap-y-2">{run.items.map(one)}</div>;
    if (run.kind === "chip") return <div key={key} className="my-2 flex flex-wrap items-center gap-2">{run.items.map(one)}</div>;
    if (run.kind === "card") return <Frame key={key} footer={streaming ? <StreamingStatus /> : undefined}><InCard.Provider value={true}>
      {runs(run.items.map((i) => i.node), false).map((inner, k) => draw({ ...inner, items: inner.items.map((i) => run.items[i.index]) }, k))}
    </InCard.Provider></Frame>;
    return run.items.map(one);
  };
  return <>{runs(list, !inCard).map(draw)}</>;
}

/** A child still arriving: a placeholder row, so a container never
 *  collapses while its references resolve. */
export const pendingNode = (n: unknown) => !n || typeof n !== "object" || !("typeName" in n);

export function PendingItems({ items }: { items: unknown }) {
  return Array.isArray(items) && items.some(pendingNode) ? <MoreComing /> : null;
}

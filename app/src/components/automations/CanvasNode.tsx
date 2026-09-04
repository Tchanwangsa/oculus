import { Handle, Position, type NodeProps, type Node } from "@xyflow/react";
import {
  inputPorts,
  isTrigger,
  nodeSummary,
  outputPorts,
  type AutomationNode,
  type Port,
  type PortType,
} from "@/lib/automations";
import { SPEC_BY_KIND } from "@/components/automations/catalog";
import { cn } from "@/lib/utils";

export type AutomationNodeData = { node: AutomationNode };
export type FlowNode = Node<AutomationNodeData, "automation">;

/** Small square handle, sized to read as a port rather than a dot. Filled
 *  ports carry data; a hollow one is just "run after this". */
const handleClass = "!h-2 !w-2 !rounded-[2px] !border !transition-colors";

const PORT_STYLE: Record<PortType, string> = {
  signal: "!border-border !bg-card hover:!bg-primary",
  files: "!border-primary/60 !bg-primary/70 hover:!bg-primary",
  summaries: "!border-primary/60 !bg-primary/70 hover:!bg-primary",
  text: "!border-muted-foreground/60 !bg-muted-foreground/50 hover:!bg-primary",
  number: "!border-muted-foreground/60 !bg-muted-foreground/50 hover:!bg-primary",
  any: "!border-border !bg-surface hover:!bg-primary",
};

/** One port row. The handle is pulled out onto the card's border by the
 *  negative offset — the node's own padding is 12px, so -12 lands it exactly
 *  on the edge whatever the label does. */
function PortRow({ port, side }: { port: Port; side: "in" | "out" }) {
  return (
    <div className={cn("relative flex h-[15px] items-center", side === "out" && "justify-end")}>
      <Handle
        id={port.id}
        type={side === "in" ? "target" : "source"}
        position={side === "in" ? Position.Left : Position.Right}
        style={side === "in" ? { left: -12 } : { right: -12 }}
        className={cn(handleClass, PORT_STYLE[port.type])}
      />
      <span className="truncate text-[10px] leading-none text-muted-foreground">
        {port.label}
      </span>
    </div>
  );
}

/**
 * One node on the canvas: icon tile, title, the configured detail beneath it,
 * and its ports — inputs down the left edge, outputs down the right — so what
 * a graph passes around is readable without opening anything.
 */
export default function CanvasNode({ data, selected }: NodeProps<FlowNode>) {
  const node = data.node;
  const spec = SPEC_BY_KIND[node.kind];
  const Icon = spec?.icon;
  const trigger = isTrigger(node.kind);
  const ins = inputPorts(node);
  const outs = outputPorts(node);

  return (
    <div
      className={cn(
        "relative w-[236px] rounded-lg border bg-card px-3 py-2.5 shadow-xs transition-colors",
        selected ? "border-primary ring-1 ring-primary/30" : "border-border hover:border-border",
      )}
    >
      <div className="flex items-start gap-2.5">
        <div
          className={cn(
            "mt-px flex size-6 shrink-0 items-center justify-center rounded-md",
            trigger ? "bg-primary/10 text-primary" : "bg-surface text-muted-foreground",
          )}
        >
          {Icon && <Icon size={13} weight="regular" />}
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate text-xs font-medium text-foreground">
            {spec?.title ?? node.kind}
          </div>
          <div className="mt-0.5 line-clamp-2 text-[11px] leading-snug text-muted-foreground">
            {nodeSummary(node)}
          </div>
        </div>
      </div>

      <div className="mt-2 grid grid-cols-2 gap-x-2 border-t border-border-subtle pt-1.5">
        <div className="flex flex-col gap-0.5">
          {ins.map((p) => (
            <PortRow key={p.id} port={p} side="in" />
          ))}
        </div>
        <div className="flex flex-col gap-0.5">
          {outs.map((p) => (
            <PortRow key={p.id} port={p} side="out" />
          ))}
        </div>
      </div>
    </div>
  );
}

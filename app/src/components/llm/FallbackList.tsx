import { useState } from "react";
import { DotsSixVertical, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { modelKey, type LlmProvider, type ModelRef } from "@/lib/db";
import { cn } from "@/lib/utils";

/** The fallback chain, in the order it is tried. Short enough (5) to reorder
 *  by hand, so it is a drag list rather than a set of numbered selects. */
export function FallbackList({
  items,
  providers,
  onReorder,
  onRemove,
}: {
  items: ModelRef[];
  providers: LlmProvider[];
  onReorder: (next: ModelRef[]) => void;
  onRemove: (m: ModelRef) => void;
}) {
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);

  const providerLabel = (id: string) =>
    providers.find((p) => p.id === id)?.label ?? "missing provider";

  const drop = (to: number) => {
    const from = dragFrom;
    setDragFrom(null);
    setDragOver(null);
    if (from == null || from === to) return;
    const next = [...items];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    onReorder(next);
  };

  if (items.length === 0) {
    return (
      <p className="text-xs text-muted-foreground py-2">
        No fallbacks. A model that cannot run will refuse the call instead.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-1 py-1">
      {items.map((m, i) => (
        <div
          key={modelKey(m)}
          draggable
          onDragStart={() => setDragFrom(i)}
          onDragEnd={() => {
            setDragFrom(null);
            setDragOver(null);
          }}
          onDragOver={(e) => {
            e.preventDefault();
            setDragOver(i);
          }}
          onDrop={(e) => {
            e.preventDefault();
            drop(i);
          }}
          className={cn(
            "flex items-center gap-2 rounded-md border border-border bg-surface px-2 py-1.5 cursor-grab active:cursor-grabbing transition-colors",
            dragFrom === i && "opacity-50",
            dragOver === i && dragFrom !== i && "border-primary",
          )}
        >
          <DotsSixVertical size={13} className="text-muted-foreground shrink-0" />
          <span className="text-[11px] text-muted-foreground tabular-nums w-3 shrink-0">
            {i + 1}
          </span>
          <span className="text-xs text-foreground truncate">{m.model}</span>
          <span className="text-[11px] text-muted-foreground truncate">
            {providerLabel(m.providerId)}
          </span>
          <div className="flex-1" />
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={`Remove ${m.model}`}
            className="text-muted-foreground hover:text-destructive"
            onClick={() => onRemove(m)}
          >
            <X size={12} />
          </Button>
        </div>
      ))}
    </div>
  );
}

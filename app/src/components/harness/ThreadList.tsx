import { useState } from "react";
import { CircleNotch, Plus, Trash } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import type { HarnessThread, Provider } from "@/lib/harness";
import type { LiveTurn } from "@/stores/harnessStore";
import { cn } from "@/lib/utils";

const PROVIDER_SHORT: Record<Provider, string> = { claude: "Claude", codex: "Codex" };

/** The conversations column: a title per thread, a spinner while it works. */
export function ThreadList({
  threads,
  activeId,
  live,
  onOpen,
  onNew,
  onDelete,
}: {
  threads: HarnessThread[];
  activeId: number | null;
  live: Record<number, LiveTurn>;
  onOpen: (id: number) => void;
  onNew: () => void;
  onDelete: (id: number) => void;
}) {
  const [confirming, setConfirming] = useState<number | null>(null);
  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-border-subtle">
      <div className="p-2">
        <Button variant="ghost" size="xs" className="w-full justify-start" onClick={onNew}>
          <Plus size={13} /> New thread
        </Button>
      </div>
      <div className="flex flex-1 flex-col gap-0.5 overflow-y-auto px-2 pb-2">
        {threads.map((t) => {
          const running = live[t.id]?.running ?? t.status === "running";
          const active = t.id === activeId;
          return (
            <div
              key={t.id}
              className={cn(
                "group/thread flex items-center gap-1.5 rounded-lg pl-2.5 pr-1.5 py-1.5 text-xs transition-colors",
                active ? "bg-accent text-foreground" : "text-muted-foreground hover:bg-accent hover:text-foreground",
              )}
            >
              <button type="button" onClick={() => onOpen(t.id)} className="min-w-0 flex-1 text-left">
                <div className="truncate">{t.title || "Untitled"}</div>
                <div className="truncate text-[10.5px] text-muted-foreground/70">
                  {PROVIDER_SHORT[t.provider]}
                  {t.model ? ` · ${t.model}` : ""}
                </div>
              </button>
              {running ? (
                <CircleNotch size={12} className="shrink-0 animate-spin text-muted-foreground" />
              ) : confirming === t.id ? (
                <button
                  type="button"
                  onClick={() => {
                    setConfirming(null);
                    onDelete(t.id);
                  }}
                  onBlur={() => setConfirming(null)}
                  className="shrink-0 rounded px-1 text-[10.5px] text-destructive hover:bg-destructive/10"
                >
                  Delete?
                </button>
              ) : (
                <button
                  type="button"
                  aria-label="Delete thread"
                  onClick={() => setConfirming(t.id)}
                  className="shrink-0 rounded p-0.5 opacity-0 transition-opacity hover:text-foreground group-hover/thread:opacity-100"
                >
                  <Trash size={12} />
                </button>
              )}
            </div>
          );
        })}
      </div>
    </aside>
  );
}

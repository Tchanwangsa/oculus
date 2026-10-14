import { useRef, useState } from "react";
import { Timer, X } from "@phosphor-icons/react";
import { PILL } from "@/components/projects/fields/pill";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import { estimateLabel, parseEstimate } from "./taskFormat";

/**
 * The estimate as a pill; its popover edits whole minutes. Enter, or closing
 * the popover, writes; Escape discards. Text that is not a number writes
 * nothing.
 */
export function EstimatePill({
  minutes,
  onCommit,
}: {
  minutes: number | null;
  onCommit: (minutes: number | null) => void;
}) {
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState("");
  const discarded = useRef(false);

  const apply = () => {
    const next = parseEstimate(draft);
    if (!discarded.current && next !== undefined && next !== minutes) onCommit(next);
  };

  const onOpenChange = (next: boolean) => {
    if (next) {
      discarded.current = false;
      setDraft(minutes == null ? "" : String(minutes));
    } else {
      apply();
    }
    setOpen(next);
  };

  const label = estimateLabel(minutes);

  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        <button
          type="button"
          title="Estimate"
          aria-label={`Estimate: ${label ?? "not set"}`}
          className={cn(PILL, label ? "text-foreground" : "text-muted-foreground")}
        >
          <Timer size={13} className="shrink-0 text-muted-foreground" />
          <span className="truncate tabular-nums">{label ?? "Estimate"}</span>
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="flex w-auto items-center gap-2 p-2">
        <input
          autoFocus
          type="number"
          inputMode="numeric"
          min={0}
          value={draft}
          aria-label="Estimate in minutes"
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") discarded.current = true;
            if (e.key === "Enter" && !e.nativeEvent.isComposing) {
              e.preventDefault();
              onOpenChange(false);
            }
          }}
          className="h-7 w-20 rounded-md border border-border-subtle bg-transparent px-2 text-xs tabular-nums text-foreground outline-none focus:border-brand/50"
        />
        <span className="text-[11px] text-muted-foreground">minutes</span>
        {minutes != null && (
          <button
            type="button"
            onClick={() => {
              discarded.current = true;
              onCommit(null);
              setOpen(false);
            }}
            className="flex cursor-pointer items-center gap-1 rounded-md px-1.5 py-1 text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <X size={11} /> Clear
          </button>
        )}
      </PopoverContent>
    </Popover>
  );
}

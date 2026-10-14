import { lazy, Suspense, useState } from "react";
import { CalendarBlank, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { fmtDay, startOfDay } from "@/lib/format/format";
import { PILL } from "../fields/pill";
import { normaliseRange, type DueRange } from "./filterRules";

const Calendar = lazy(() =>
  import("@/components/ui/calendar").then((module) => ({ default: module.Calendar })),
);

/** A day pill over a popover calendar; the popover's Clear opens that end. */
function DayPill({
  label,
  value,
  onPick,
}: {
  label: string;
  /** Local midnight, or `null` for open. */
  value: number | null;
  onPick: (ms: number | null) => void;
}) {
  const [open, setOpen] = useState(false);
  const current = value == null ? undefined : new Date(value);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={`${label}: ${current ? fmtDay(current) : "open"}`}
          className={cn(PILL, "min-w-0 flex-1 shrink justify-start", current ? "text-foreground" : "text-muted-foreground")}
        >
          <CalendarBlank size={13} className="shrink-0 text-muted-foreground" />
          <span className="truncate">{current ? fmtDay(current) : label}</span>
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-auto p-0">
        {open && (
          <Suspense fallback={<div className="h-[193px] w-[184px]" />}>
            <Calendar
              mode="single"
              selected={current}
              defaultMonth={current}
              onSelect={(day) => {
                if (!day) return;
                onPick(startOfDay(day).getTime());
                setOpen(false);
              }}
              autoFocus
            />
          </Suspense>
        )}
        {current && (
          <div className="flex justify-end border-t border-border-subtle px-2 py-1.5">
            <button
              type="button"
              onClick={() => {
                onPick(null);
                setOpen(false);
              }}
              className="flex cursor-pointer items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            >
              <X size={11} /> Clear
            </button>
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}

/**
 * The Custom due range: a From and a To day, either optional. A To before the
 * From swaps them (`normaliseRange`), so both picks survive.
 */
export function DueRangeFields({ range, onChange }: { range: DueRange; onChange: (next: DueRange) => void }) {
  return (
    <div className="mt-2 flex items-center gap-1.5">
      <DayPill label="From" value={range.from} onPick={(from) => onChange(normaliseRange({ ...range, from }))} />
      <span className="shrink-0 text-xs text-muted-foreground">–</span>
      <DayPill label="To" value={range.to} onPick={(to) => onChange(normaliseRange({ ...range, to }))} />
    </div>
  );
}

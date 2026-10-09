import { lazy, Suspense, useState } from "react";
import { CalendarBlank, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { sqliteUtcToMs } from "@/lib/format/format";

const Calendar = lazy(() =>
  import("@/components/ui/calendar").then((module) => ({ default: module.Calendar })),
);

/** What a date with no time yet gets: a deadline lands at the end of its day,
 *  a start at the beginning of the working one. */
const DEFAULT_TIME: Record<"start" | "end", { h: number; m: number }> = {
  start: { h: 9, m: 0 },
  end: { h: 23, m: 59 },
};

/** "15 Sep, 11:59 pm", with the year when it is not this one (which
 *  `fmtClock` never prints). */
function label(d: Date): string {
  const opts: Intl.DateTimeFormatOptions = { day: "numeric", month: "short" };
  if (d.getFullYear() !== new Date().getFullYear()) opts.year = "numeric";
  const time = d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  return `${d.toLocaleDateString([], opts)}, ${time}`;
}

/** `HH:mm`: the time input's value is 24-hour whatever it displays. */
function clockValue(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** Picked in a popover — see docs/ui.md (no native date inputs). */
export function DateTimeField({
  value,
  onCommit,
  defaultTime = "end",
  placeholder = "Empty",
  className,
}: {
  /** As stored: SQLite's naive UTC, or an ISO instant. `null` is unset. */
  value: string | null;
  /** A full ISO 8601 instant (`check_iso8601`'s contract), `null` when cleared. */
  onCommit: (iso: string | null) => void;
  defaultTime?: "start" | "end";
  placeholder?: string;
  className?: string;
}) {
  const [open, setOpen] = useState(false);

  const ms = sqliteUtcToMs(value);
  const current = ms == null ? null : new Date(ms);

  // A `Date` built from local parts is the instant meant; `toISOString` is the
  // only place a zone is applied. Never round-trip via `toISOString().slice()`.
  const commit = (d: Date) => onCommit(d.toISOString());

  const pickDay = (day: Date | undefined) => {
    if (!day) return;
    const time = current ?? null;
    const { h, m } = DEFAULT_TIME[defaultTime];
    const next = new Date(day);
    next.setHours(time ? time.getHours() : h, time ? time.getMinutes() : m, 0, 0);
    commit(next);
  };

  // An emptied time input keeps the date; Clear is what means "no date".
  const pickTime = (raw: string) => {
    const [h, m] = raw.split(":").map(Number);
    if (!Number.isFinite(h) || !Number.isFinite(m)) return;
    const next = new Date(current ?? new Date());
    next.setHours(h, m, 0, 0);
    commit(next);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn(
            "inline-flex cursor-pointer items-center gap-1.5 rounded-md border border-transparent px-1.5 py-1 text-xs transition-colors",
            "hover:border-border-subtle hover:bg-surface",
            open && "border-border-subtle bg-surface",
            current ? "text-foreground" : "text-muted-foreground/60",
            className,
          )}
        >
          <CalendarBlank size={13} className="shrink-0 text-muted-foreground" />
          {current ? label(current) : placeholder}
        </button>
      </PopoverTrigger>

      <PopoverContent align="start" className="w-auto p-0">
        {open && (
          /* The calendar's size from its `--cell-size` (24px): seven columns
             plus `p-2` wide, and caption, weekdays and five weeks tall. */
          <Suspense fallback={<div className="h-[193px] w-[184px]" />}>
            <Calendar
              mode="single"
              selected={current ?? undefined}
              defaultMonth={current ?? undefined}
              onSelect={pickDay}
              autoFocus
            />
          </Suspense>
        )}
        <div className="flex items-center gap-2 border-t border-border-subtle px-2 py-1.5">
          <span className="text-[11px] text-muted-foreground">Time</span>
          <input
            type="time"
            value={current ? clockValue(current) : ""}
            onChange={(e) => pickTime(e.target.value)}
            className={cn(
              "rounded-md border border-border-subtle bg-transparent px-1.5 py-0.5 text-[11px] text-foreground outline-none",
              "transition-colors focus:border-brand/40 focus:bg-card",
            )}
          />
          <span className="flex-1" />
          {current && (
            <button
              type="button"
              onClick={() => {
                onCommit(null);
                setOpen(false);
              }}
              className="flex cursor-pointer items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            >
              <X size={11} /> Clear
            </button>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}

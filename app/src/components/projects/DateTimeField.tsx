import { useState } from "react";
import { CalendarBlank, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Calendar } from "@/components/ui/calendar";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { sqliteUtcToMs } from "@/lib/format";

/**
 * A date and a time, picked rather than typed.
 *
 * This was an `<input type="datetime-local">`, which is the obvious answer and
 * the wrong one on macOS: WebKit renders it as a row of separate editable
 * segments, greys the ones it considers unfilled, and lights each under the
 * pointer on its own — so a single field reads as five controls at three
 * different weights, and its popup calendar is the OS's, in the OS's calendar
 * system, ignoring every token in `index.css`. (The screenshot that prompted
 * this had it rendering a Buddhist-era year.) It also could not be committed
 * on change, because a half-typed segment reports the whole field as empty.
 *
 * So the field is a button showing what is set, and the editing happens in a
 * popover we own: shadcn's `Calendar` for the date and one `type="time"` input
 * for the clock, which is the one native control here with nothing to mis-render
 * — it is two segments, and they are never empty because picking a date always
 * sets a time.
 */

/** What a date with no time yet gets. A deadline lands at the end of its day
 *  and a start at the beginning of the working one — picking "the 20th" for a
 *  due date means the 20th, not one minute past midnight on it. */
const DEFAULT_TIME: Record<"start" | "end", { h: number; m: number }> = {
  start: { h: 9, m: 0 },
  end: { h: 23, m: 59 },
};

/**
 * "15 Sep, 11:59 pm", with the year only once it is not this one.
 *
 * `fmtClock` is the app's version of this and is deliberately not used: it
 * never prints a year, which is right for a timeline entry and wrong for a due
 * date, where next February and last February are the whole question.
 */
function label(d: Date): string {
  const opts: Intl.DateTimeFormatOptions = { day: "numeric", month: "short" };
  if (d.getFullYear() !== new Date().getFullYear()) opts.year = "numeric";
  const time = d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  return `${d.toLocaleDateString([], opts)}, ${time}`;
}

/** `HH:mm` for the time input, which wants 24-hour regardless of how it
 *  chooses to display it. */
function clockValue(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function DateTimeField({
  value,
  onCommit,
  defaultTime = "end",
  placeholder = "Empty",
  className,
}: {
  /** As stored: SQLite's naive UTC, or an ISO instant. `null` is unset. */
  value: string | null;
  /** A full ISO 8601 instant, or `null` when cleared. `check_iso8601` in
   *  `app/src-tauri/src/projects.rs` is the contract, and takes this shape —
   *  so a date set here and one passed to `oculus task update --due` store
   *  identically. */
  onCommit: (iso: string | null) => void;
  defaultTime?: "start" | "end";
  placeholder?: string;
  className?: string;
}) {
  const [open, setOpen] = useState(false);

  const ms = sqliteUtcToMs(value);
  const current = ms == null ? null : new Date(ms);

  /**
   * Write a local wall-clock date back out as an instant.
   *
   * The whole conversion lives here, in one direction, which is the point of
   * the rewrite: a `Date` built from local parts *is* the instant the user
   * meant, and `toISOString` is the only place a zone is applied. The old
   * field had to go both ways through a `YYYY-MM-DDTHH:mm` string, where
   * `toISOString().slice(0, 16)` is the tempting and wrong way back — it
   * renders UTC, so an 11pm Melbourne deadline read into the field as midday
   * and saving it moved the date.
   */
  const commit = (d: Date) => onCommit(d.toISOString());

  /** A day from the calendar, keeping whatever clock is already set. */
  const pickDay = (day: Date | undefined) => {
    if (!day) return;
    const time = current ?? null;
    const { h, m } = DEFAULT_TIME[defaultTime];
    const next = new Date(day);
    next.setHours(time ? time.getHours() : h, time ? time.getMinutes() : m, 0, 0);
    commit(next);
  };

  /** A clock from the time input, keeping whatever day is already set. An
   *  emptied time input keeps the date rather than clearing the field — Clear
   *  is the control that means "no date", and it is right there. */
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
            // One control, one hover state — which is the other half of what
            // was wrong with the native field.
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
        <Calendar
          mode="single"
          selected={current ?? undefined}
          // Opens on the month you are editing rather than on today, which is
          // the difference between confirming a date and hunting for it.
          defaultMonth={current ?? undefined}
          onSelect={pickDay}
          autoFocus
        />
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

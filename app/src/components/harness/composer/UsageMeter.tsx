import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { fmtTime } from "@/lib/format/format";
import type { RateWindow, ThreadUsage } from "@/lib/harness";
import { cn } from "@/lib/utils";

/** Past this, a bar stops being informational and starts being a warning. */
const HOT = 90;
const WARM = 75;

/** Tokens as the model counts them: `106k`, `1M`, `1.5M`. */
function fmtTokens(n: number) {
  if (n >= 1_000_000) {
    const m = n / 1_000_000;
    return `${Number.isInteger(m) ? m : Number(m.toFixed(1))}M`;
  }
  return n >= 1000 ? `${Math.round(n / 1000)}k` : String(n);
}

/** "Resets in 3 hr 48 min" under a day out, else "Resets Tue 9:00 pm". */
function fmtResets(unixSeconds: number | null): string | null {
  if (!unixSeconds) return null;
  const ms = unixSeconds * 1000;
  const left = ms - Date.now();
  if (left <= 0) return null;
  if (left < 86_400_000) {
    const mins = Math.round(left / 60_000);
    if (mins < 60) return `Resets in ${Math.max(1, mins)} min`;
    const h = Math.floor(mins / 60);
    const m = mins % 60;
    return m ? `Resets in ${h} hr ${m} min` : `Resets in ${h} hr`;
  }
  const d = new Date(ms);
  return `Resets ${d.toLocaleDateString([], { weekday: "short" })} ${fmtTime(ms)}`;
}

function tint(percent: number) {
  if (percent >= HOT) return "bg-destructive";
  if (percent >= WARM) return "bg-warning";
  return "bg-brand";
}

/** A 16px donut for the control row. */
function Ring({ percent }: { percent: number }) {
  const r = 6.5;
  const circumference = 2 * Math.PI * r;
  const p = Math.max(0, Math.min(100, percent));
  return (
    <svg viewBox="0 0 18 18" className="size-4" aria-hidden>
      <circle cx="9" cy="9" r={r} fill="none" strokeWidth="3" className="stroke-border" />
      {p > 0 && (
        <circle
          cx="9"
          cy="9"
          r={r}
          fill="none"
          strokeWidth="3"
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={circumference * (1 - p / 100)}
          transform="rotate(-90 9 9)"
          className={cn(
            "transition-[stroke-dashoffset] duration-300",
            p >= HOT ? "stroke-destructive" : p >= WARM ? "stroke-warning" : "stroke-brand",
          )}
        />
      )}
    </svg>
  );
}

function Meter({
  label,
  note,
  value,
  percent,
}: {
  label: string;
  note?: string | null;
  /** Shown instead of the percentage when there is a truer number to show. */
  value?: string;
  percent: number;
}) {
  const p = Math.max(0, Math.min(100, percent));
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-baseline gap-2 text-[11px]">
        <span className="shrink-0 text-foreground">{label}</span>
        {note && <span className="min-w-0 truncate text-[10.5px] text-muted-foreground">{note}</span>}
        <span className="ml-auto shrink-0 tabular-nums text-muted-foreground">
          {value ?? `${Math.round(p)}%`}
        </span>
      </div>
      <div className="h-1 w-full overflow-hidden rounded-full bg-secondary">
        <div
          className={cn("h-full rounded-full transition-[width] duration-300", tint(p))}
          style={{ width: `${p}%` }}
        />
      </div>
    </div>
  );
}

/**
 * The ring is this thread's context usage only (never the worst of several
 * numbers); the popover adds the account's rate-limit windows. No spend shown:
 * subscription CLIs report a notional price nobody is billed.
 */
export function UsageMeter({
  usage,
  rateLimits,
}: {
  usage: ThreadUsage | null;
  rateLimits: RateWindow[];
}) {
  const used = usage?.contextTokens ?? null;
  const limit = usage?.contextWindow ?? null;
  const contextPercent = used && limit ? (used / limit) * 100 : 0;

  if (used == null && rateLimits.length === 0) return null;

  const summary = [
    used != null && limit
      ? `${Math.round(contextPercent)}% of context`
      : used != null
        ? `${fmtTokens(used)} context tokens`
        : null,
    ...rateLimits.map((w) => `${w.label} ${Math.round(w.used_percent)}%`),
  ].filter(Boolean);

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={`Usage — ${summary.join(", ")}`}
          title={summary.join(" · ")}
          className="shrink-0 text-muted-foreground hover:text-foreground"
        >
          <Ring percent={contextPercent} />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" side="top" className="w-64 p-3">
        <div className="flex flex-col gap-3">
          {used != null && (
            <Meter
              label="Context window"
              value={limit ? `${fmtTokens(used)} / ${fmtTokens(limit)}` : fmtTokens(used)}
              percent={contextPercent}
            />
          )}
          {rateLimits.length > 0 && (
            <div className="flex flex-col gap-2.5">
              <div className="text-[10.5px] text-muted-foreground">Plan usage limits</div>
              {rateLimits.map((w) => (
                <Meter
                  key={w.label}
                  label={w.label}
                  note={fmtResets(w.resets_at)}
                  percent={w.used_percent}
                />
              ))}
            </div>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}

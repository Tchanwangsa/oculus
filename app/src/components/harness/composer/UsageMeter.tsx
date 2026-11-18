import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { fmtResets, nearestLimit, type RateWindow, type ThreadUsage } from "@/lib/harness";
import { cn } from "@/lib/utils";

/** Past this, a bar stops being informational and starts being a warning. */
const HOT = 90;
const WARM = 75;

/** Tokens as the model counts them: `106k`, `1M`, `1.5M`. */
export function fmtTokens(n: number) {
  if (n >= 1_000_000) {
    const m = n / 1_000_000;
    return `${Number.isInteger(m) ? m : Number(m.toFixed(1))}M`;
  }
  return n >= 1000 ? `${Math.round(n / 1000)}k` : String(n);
}

/** A context or rate bar's fill: brand, then warning, then destructive. */
export function usageTint(percent: number) {
  if (percent >= HOT) return "bg-destructive";
  if (percent >= WARM) return "bg-warning";
  return "bg-brand";
}

/** A 16px donut for the control row. */
export function Ring({ percent }: { percent: number }) {
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
  percent,
}: {
  label: string;
  note?: string | null;
  percent: number;
}) {
  const p = Math.max(0, Math.min(100, percent));
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-baseline gap-2 text-[11px]">
        <span className="shrink-0 text-foreground">{label}</span>
        {note && <span className="min-w-0 truncate text-[10.5px] text-muted-foreground">{note}</span>}
        <span className="ml-auto shrink-0 tabular-nums text-muted-foreground">
          {`${Math.round(p)}%`}
        </span>
      </div>
      <div className="h-1 w-full overflow-hidden rounded-full bg-secondary">
        <div
          className={cn("h-full rounded-full transition-[width] duration-300", usageTint(p))}
          style={{ width: `${p}%` }}
        />
      </div>
    </div>
  );
}

/**
 * What a thread's usage reads as. Usage is the plan's limits only: the ring
 * and its one-line reading (`headline`, `ringLabel`) are the window nearest
 * its limit (`nearestLimit`), the session's or the week's, and `parts` lists
 * every window. The context window is reported beside it (`used`, `limit`,
 * `contextPercent`) for the settings list's `ContextStrip` and the thread
 * info card, never inside usage.
 */
export function usageSummary(usage: ThreadUsage | null, rateLimits: RateWindow[]) {
  const used = usage?.contextTokens ?? null;
  const limit = usage?.contextWindow ?? null;
  const contextPercent = used && limit ? (used / limit) * 100 : 0;
  const nearest = nearestLimit(rateLimits);
  const nearestPercent = nearest ? Math.round(nearest.used_percent) : 0;
  return {
    used,
    limit,
    contextPercent,
    nearest,
    /** What the ring fills to. */
    percent: nearest ? nearest.used_percent : 0,
    /** The ring's short reading: "Weekly 62%". */
    headline: nearest ? `${nearest.label} ${nearestPercent}%` : null,
    /** The ring named in full, for its title and label. */
    ringLabel: nearest ? `${nearest.label} limit ${nearestPercent}% used` : null,
    parts: rateLimits.map((w) => `${w.label} ${Math.round(w.used_percent)}%`),
  };
}

/**
 * The ring is the plan window closest to running out, so the number that
 * stops the student first is the one in view; an agent without plan windows
 * (opencode, Antigravity) shows none. The popover lists every window. No
 * spend shown: subscription CLIs report a notional price nobody is billed.
 */
export function UsageMeter({
  usage,
  rateLimits,
}: {
  usage: ThreadUsage | null;
  rateLimits: RateWindow[];
}) {
  const summary = usageSummary(usage, rateLimits);
  if (!summary.nearest) return null;

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={`Usage — ${summary.ringLabel}`}
          title={summary.ringLabel ?? undefined}
          className="shrink-0 text-muted-foreground hover:text-foreground"
        >
          <Ring percent={summary.percent} />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" side="top" className="w-64 p-3">
        <UsageDetails rateLimits={rateLimits} />
      </PopoverContent>
    </Popover>
  );
}

/** The meters inside a usage popover: each plan window with its reset. */
export function UsageDetails({ rateLimits }: { rateLimits: RateWindow[] }) {
  return (
    <div className="flex flex-col gap-2.5">
      <div className="text-[10.5px] text-muted-foreground">Plan usage limits</div>
      {rateLimits.map((w) => (
        <Meter key={w.label} label={w.label} note={fmtResets(w.resets_at)} percent={w.used_percent} />
      ))}
    </div>
  );
}

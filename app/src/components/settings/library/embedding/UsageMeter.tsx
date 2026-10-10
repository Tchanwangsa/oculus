import { Info } from "@phosphor-icons/react";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { StatRow } from "@/components/settings/shared/section";
import { si } from "./format";
import type { VoyageUsage } from "./types";

/**
 * What plan this account is on. May be "unknown": limits are only detected
 * from 429 bodies during a run (`embed/voyage/ledger/`).
 */
export function PlanRow({ usage }: { usage: VoyageUsage }) {
  const perMinute = `${si(usage.tpm)} tokens/min`;
  return (
    <StatRow
      label="Voyage plan"
      value={
        usage.plan === "free"
          ? `No payment method · ${perMinute}`
          : usage.plan === "paid"
            ? `Payment method on file · ${perMinute}`
            : "Not measured yet"
      }
      hint={
        usage.plan === "unknown"
          ? "Voyage has no usage API. The plan is learned from the first requests a run makes."
          : "Detected from Voyage's own rate-limit responses."
      }
    />
  );
}

/**
 * The free pixel grant as a meter: what has been spent, what this run would
 * add, and the spend guard as a mark on the same track. Figures are Oculus's
 * own count (see `VoyageUsage`).
 */
export function AllowanceMeter({
  usage,
  runPixels,
  onChange,
}: {
  usage: VoyageUsage;
  /** This run's projection, drawn ahead of the fill. 0 when unknown. */
  runPixels: number;
  onChange: (percent: number) => void;
}) {
  const grant = Math.max(1, usage.free_pixels);
  const spent = (usage.pixels / grant) * 100;
  const projected = Math.min((runPixels / grant) * 100, Math.max(0, 100 - spent));

  return (
    <div className="py-2">
      <div className="flex items-baseline justify-between gap-4">
        <p className="flex items-center gap-1 text-xs text-foreground">
          Free allowance
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="text-muted-foreground" aria-label="Where this number comes from">
                <Info size={12} weight="bold" />
              </span>
            </TooltipTrigger>
            <TooltipContent>
              No usage API — Oculus&rsquo;s own count of what it sent
            </TooltipContent>
          </Tooltip>
        </p>
        <p className="text-xs text-foreground tabular-nums">
          {si(usage.pixels)} of {si(usage.free_pixels)} pixels ·{" "}
          {spent < 0.1 && usage.pixels > 0 ? "<0.1" : spent.toFixed(1)}%
        </p>
      </div>

      <div className="relative mt-2 h-2.5 overflow-hidden rounded-full bg-surface">
        <div className="absolute inset-0 flex">
          <div
            className="h-full bg-chart-1"
            style={{ width: `${Math.min(spent, 100)}%`, minWidth: usage.pixels > 0 ? 3 : 0 }}
          />
          {projected > 0 ? (
            <div className="h-full bg-chart-1/35" style={{ width: `${projected}%`, minWidth: 3 }} />
          ) : null}
        </div>
        {usage.stop_at_percent > 0 && usage.stop_at_percent < 100 ? (
          <div
            className="absolute top-0 h-full w-[2px] bg-destructive"
            style={{ left: `${usage.stop_at_percent}%` }}
          />
        ) : null}
      </div>

      <div className="mt-2 flex items-center justify-between gap-4">
        <p className="text-[11px] text-muted-foreground">
          {runPixels > 0 ? `This run adds ${si(runPixels)}. ` : ""}
          {usage.stop_at_percent === 0
            ? "No limit — Voyage charges past 100%."
            : usage.stop_at_percent === 100
              ? "Stops before Voyage starts charging."
              : "Stops at the mark."}
        </p>
        <Select
          value={String(usage.stop_at_percent)}
          onValueChange={(value) => onChange(Number(value))}
        >
          <SelectTrigger aria-label="Stop indexing at" size="sm" className="h-7 w-32 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {[50, 75, 90, 100].map((option) => (
              <SelectItem key={option} value={String(option)} className="text-xs">
                Stop at {option}%
              </SelectItem>
            ))}
            <SelectItem value="0" className="text-xs">
              No limit
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
  );
}

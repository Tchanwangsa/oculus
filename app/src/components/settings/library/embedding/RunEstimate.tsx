import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowSquareOut } from "@phosphor-icons/react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { cn } from "@/lib/utils";
import { roughly, si } from "./format";
import type { EmbedEstimate, VoyageUsage } from "./types";

/** Where a payment method is added. Linked, not described. */
const VOYAGE_DASHBOARD = "https://dashboard.voyageai.com/";

/** Colour follows the file kind, never its current rank. */
const KIND_COLOR: Record<string, string> = {
  pdf: "bg-chart-1",
  docx: "bg-chart-2",
  pptx: "bg-chart-3",
  doc: "bg-chart-4",
  ppt: "bg-chart-5",
};

/**
 * What pressing Index will cost, in time and dollars — measured by Rust
 * (`app/src-tauri/src/embed/estimate.rs`). The free pixel grant applies to
 * every account, so a payment method buys speed, not price: when tier 1 is
 * faster the headline is the time saved.
 */
export function RunEstimate({
  estimate,
  estimating,
  usage,
}: {
  estimate: EmbedEstimate | null;
  estimating: boolean;
  usage: VoyageUsage | null;
}) {
  if (estimating && !estimate) {
    return (
      <p className="pt-2 text-[11px] text-muted-foreground">Measuring what is outstanding…</p>
    );
  }
  if (!estimate || estimate.files === 0 || estimate.pages === 0) return null;

  // Never pitch an upgrade off an `assumed` tier.
  const upgrade =
    estimate.tier_free &&
    estimate.tier_source !== "assumed" &&
    estimate.seconds_tier1 < estimate.seconds / 2;
  const cut = estimate.stops_after_pages;
  const kinds = estimate.kinds.filter((bucket) => bucket.pages > 0);
  const totalPages = Math.max(1, estimate.pages);

  return (
    <Alert
      variant={cut != null || estimate.cost_usd > 0 ? "warning" : "default"}
      className="mt-3"
    >
      <AlertTitle className="text-xs">
        {upgrade
          ? `Save ${roughly(estimate.seconds).replace(/^about /, "")} of indexing, at no extra cost`
          : `${roughly(estimate.seconds)} to index ${estimate.pages.toLocaleString()} pages${
              estimate.tier_source === "assumed" ? ", if this account is on tier 1" : ""
            }`}
      </AlertTitle>
      <AlertDescription className="gap-2 text-[11px]">
        {upgrade ? (
          <div className="flex w-full items-center gap-3">
            <span className="tabular-nums">
              {roughly(estimate.seconds)} now · {roughly(estimate.seconds_tier1)} on tier 1
            </span>
            <button
              type="button"
              className="inline-flex items-center gap-1 text-brand hover:underline"
              onClick={() => void openUrl(VOYAGE_DASHBOARD)}
            >
              Add a payment method
              <ArrowSquareOut size={11} weight="bold" />
            </button>
          </div>
        ) : null}

        <div className="w-full">
          <div className="flex h-1.5 gap-[2px] overflow-hidden rounded-full bg-surface">
            {kinds.map((bucket) => (
              <div
                key={bucket.label}
                className={cn("h-full", KIND_COLOR[bucket.label] ?? "bg-chart-other")}
                style={{ width: `${(bucket.pages / totalPages) * 100}%`, minWidth: 4 }}
              />
            ))}
          </div>
          <div className="mt-1.5 flex flex-wrap gap-x-4 gap-y-1">
            {kinds.map((bucket) => (
              <span key={bucket.label} className="flex items-center gap-1.5 tabular-nums">
                <span
                  className={cn(
                    "size-2 shrink-0 rounded-[3px]",
                    KIND_COLOR[bucket.label] ?? "bg-chart-other",
                  )}
                />
                {bucket.label.toUpperCase()} {bucket.files}
                <span className="text-muted-foreground">
                  {bucket.pages.toLocaleString()} pages
                </span>
              </span>
            ))}
          </div>
        </div>

        <p className="tabular-nums">
          Estimated cost{" "}
          <span className="text-foreground">
            {estimate.cost_usd > 0 ? `$${estimate.cost_usd.toFixed(2)}` : "Free"}
          </span>{" "}
          · {si(estimate.pixels)} of {si(usage?.free_pixels ?? 150e9)} pixels (
          {((estimate.pixels / Math.max(1, usage?.free_pixels ?? 150e9)) * 100).toFixed(1)}%)
        </p>

        {cut != null ? (
          <p className="font-medium">
            Stops after {cut.toLocaleString()} of {estimate.pages.toLocaleString()} pages at
            the spend limit above.
          </p>
        ) : null}

        {estimate.unreadable > 0 ? (
          <p className="font-medium">
            {estimate.unreadable} file{estimate.unreadable === 1 ? "" : "s"} could not be
            measured and may fail.
          </p>
        ) : null}
      </AlertDescription>
    </Alert>
  );
}

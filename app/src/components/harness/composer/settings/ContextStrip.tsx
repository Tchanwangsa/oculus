import { cn } from "@/lib/utils";
import { fmtTokens, usageTint } from "../UsageMeter";

/**
 * The foot of the settings list: how full this chat's context window is, as
 * a bar and its tokens, always in view beside the plan limit on the Usage
 * row. Without a known window it shows the tokens alone.
 */
export function ContextStrip({ used, limit, percent }: { used: number; limit: number | null; percent: number }) {
  const p = Math.max(0, Math.min(100, percent));
  const reading = limit ? `${fmtTokens(used)} / ${fmtTokens(limit)}` : `${fmtTokens(used)} tokens`;
  return (
    <div
      className="mx-1 mb-1 flex items-center gap-2.5 rounded-md bg-surface px-2 py-1.5 text-xs"
      title={limit ? `Context: ${Math.round(p)}% used` : `Context: ${reading}`}
    >
      <span className="shrink-0 text-muted-foreground">Context</span>
      {limit ? (
        <span aria-hidden className="h-1 min-w-8 flex-1 overflow-hidden rounded-full bg-border">
          <span
            className={cn("block h-full rounded-full transition-[width] duration-300", usageTint(p))}
            style={{ width: `${p}%` }}
          />
        </span>
      ) : (
        <span className="flex-1" />
      )}
      <span className="shrink-0 tabular-nums text-muted-foreground">{reading}</span>
    </div>
  );
}

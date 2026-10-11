import { X } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { dismissJobNotices, type JobId } from "@/lib/db";
import type { PickerProvider } from "@/lib/harness";
import { useJobAttention } from "@/stores/agents/jobAttentionStore";
import { agentLabel, modelLabel } from "./chain";

/** What the audit found for one job: models it removed (dismissable) and
 *  models whose agent CLI is missing (shown while it stays missing). */
export function JobAttention({
  job,
  off,
  providers,
}: {
  job: JobId;
  /** The chain is empty, so the job doesn't run. */
  off: boolean;
  providers: PickerProvider[];
}) {
  const notices = useJobAttention((s) => s.notices);
  const flagged = useJobAttention((s) => s.flagged);
  const mine = notices.filter((n) => n.job === job);
  const skipped = flagged.filter((f) => f.job === job);
  if (mine.length === 0 && skipped.length === 0) return null;

  const lines = [
    ...mine.map(
      (n) =>
        `${n.label} is no longer offered by ${agentLabel(providers, n.provider)}, so it was removed.` +
        (off ? " This job is off until you choose a model." : ""),
    ),
    ...skipped.map(
      (f) =>
        `${agentLabel(providers, f.provider)} isn't installed, so ${modelLabel(providers, f.provider, f.model)} is skipped.`,
    ),
  ];

  return (
    <div className="mt-1 flex items-start gap-1">
      <div className="min-w-0 space-y-0.5 text-xs text-destructive">
        {lines.map((line, i) => (
          <p key={i}>{line}</p>
        ))}
      </div>
      {mine.length > 0 && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="Dismiss"
              onClick={() => void dismissJobNotices(job)}
              className="-my-1 text-muted-foreground hover:text-foreground"
            >
              <X size={12} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Dismiss</TooltipContent>
        </Tooltip>
      )}
    </div>
  );
}

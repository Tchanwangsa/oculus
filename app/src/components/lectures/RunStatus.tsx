import { useEffect, useState, type ReactNode } from "react";
import { CircleNotch } from "@phosphor-icons/react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { fmtClockSecs } from "@/lib/media";
import { toolVerb, type ToolKind } from "@/lib/harness";
import { useTabActive } from "@/components/tabs/TabContext";

/** The pieces the chapter list and the reading copy share for an agent job:
 *  the in-flight status and the Regenerate footer. Their empty state is the
 *  dock's `PanelEmpty` (`components/media/MediaDock.tsx`). */

interface StepProgress {
  phase: string;
  detail: string | null;
  kind: ToolKind | null;
  done: number | null;
  total: number | null;
}

/** What the job is touching right now: a tool call in the timeline's words
 *  (`toolVerb`), else how far through a countable phase it is. */
function stepDetail(p: StepProgress): string | null {
  const title = p.detail?.trim();
  if (title) return p.kind ? `${toolVerb(p.kind, false)} ${title}` : title;
  if (p.kind) return toolVerb(p.kind, false);
  if (p.done !== null && p.total) {
    // The decode counts frames off the file; the total is Echo360's duration,
    // which runs a few seconds short.
    const done = Math.min(p.done, p.total);
    return p.phase === "decoding"
      ? `${fmtClockSecs(done)} of ${fmtClockSecs(p.total)}`
      : `${done} of ${p.total}`;
  }
  return null;
}

/**
 * A run in flight: phase, elapsed clock, and the current step. No progress bar
 * — a bar that fills and keeps waiting is what hung looks like. A run already
 * in flight at app launch has no start time or step until its next event, so
 * the spinner alone is a valid state.
 */
export function RunStatus({
  since,
  label,
  progress,
  counter,
  note,
  compact,
}: {
  since: number | null;
  label: string;
  progress: StepProgress | null;
  /** Shown between the label and the clock, e.g. "3/7". */
  counter?: ReactNode;
  /** The expected duration, under the status when not `compact`. */
  note: string;
  compact?: boolean;
}) {
  const active = useTabActive();
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (since === null || !active) return;
    setNow(Date.now());
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [since, active]);

  const elapsed = since === null ? null : Math.max(0, Math.floor((now - since) / 1000));
  const detail = progress ? stepDetail(progress) : null;

  return (
    <div
      className={cn(
        "flex w-full min-w-0 flex-col gap-0.5 text-[11px] text-brand",
        !compact && "items-center text-center",
      )}
    >
      <span className="flex max-w-full items-center gap-1.5">
        <CircleNotch size={12} className="shrink-0 animate-spin" />
        <span className="truncate">{label}</span>
        {counter != null && (
          <span className="shrink-0 tabular-nums text-muted-foreground">{counter}</span>
        )}
        {elapsed !== null && (
          <span className="shrink-0 tabular-nums text-muted-foreground">{fmtClockSecs(elapsed)}</span>
        )}
      </span>
      {detail && (
        <span className="block max-w-full truncate text-muted-foreground" title={detail}>
          {detail}
        </span>
      )}
      {!compact && <span className="text-muted-foreground">{note}</span>}
    </div>
  );
}

/** Regenerate, with the last run's error beside it: a failed regenerate keeps
 *  what was there, so the failure sits next to it rather than in its place. */
export function RegenerateRow({
  disabled,
  title,
  error,
  onClick,
}: {
  disabled: boolean;
  title?: string;
  error: string | null;
  onClick: () => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <Button
        size="xs"
        variant="ghost"
        disabled={disabled}
        title={title}
        onClick={onClick}
        className="-ml-1 text-muted-foreground"
      >
        Regenerate
      </Button>
      {error && (
        <span className="min-w-0 flex-1 truncate text-[10px] text-destructive" title={error}>
          {error}
        </span>
      )}
    </div>
  );
}

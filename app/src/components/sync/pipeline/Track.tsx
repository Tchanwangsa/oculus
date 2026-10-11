import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { PipelineItem, StageState, StatusView } from "@/stores/sync/pipelineStore";
import { HATCH } from "@/components/sync/pipeline/constants";
import { hintOf } from "@/components/sync/pipeline/captions";
import { isRateLimited, stageKeys, uploadWaiting } from "@/components/sync/pipeline/facts";

/** How a segment draws: the stage's state, with the live stage's percent. */
function Segment({
  state,
  percent,
  held,
  paused,
}: {
  state: StageState;
  /** Only for the moving stage; null draws an indeterminate pulse. */
  percent: number | null;
  /** In flight but not moving (waiting for its upload turn, rate-limited). */
  held: boolean;
  /** The stage a paused file stops at. */
  paused: boolean;
}) {
  let fill: ReactNode = null;
  let track = "bg-secondary";
  if (paused) track = "bg-warning/50";
  else if (state === "done") fill = <span className="absolute inset-0 bg-success/80" />;
  else if (state === "error") fill = <span className="absolute inset-0 bg-destructive" />;
  else if (state === "queued") track = "bg-brand/20";
  else if (state === "active" && percent != null) {
    track = "bg-brand/20";
    fill = (
      <span
        className={cn(
          "absolute inset-y-0 left-0 transition-[width] duration-500",
          held ? "bg-warning/70" : "bg-brand",
        )}
        style={{ width: `${Math.max(4, Math.min(100, percent))}%` }}
      />
    );
  } else if (state === "active") {
    track = held ? "bg-brand/20" : "bg-brand/40 animate-pulse will-change-[opacity]";
  }
  return (
    <span
      className={cn("relative block h-1.5 flex-1 overflow-hidden rounded-full", track)}
      style={state === "skipped" ? HATCH : undefined}
    >
      {fill}
    </span>
  );
}

/** The row's status: one segment per stage. Its tooltip is the whole
 *  caption, which the row drops when the table is narrow. */
export function Track({
  item,
  s,
  embedStage,
}: {
  item: PipelineItem;
  s: StatusView;
  embedStage: boolean;
}) {
  const held = uploadWaiting(item) || isRateLimited(item);
  const keys = stageKeys(embedStage);
  const pausedAt = s.phase === "paused" ? keys.find((k) => item[k] !== "done") : undefined;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        {/* The padding is the hover target; the bar itself is 6px. */}
        <div className="flex w-24 shrink-0 items-center gap-[3px] py-1.5">
          {keys.map((key) => {
            const state = item[key];
            const moving = state === "active" && s.phase === "active";
            return (
              <Segment
                key={key}
                state={state}
                percent={moving ? s.percent : null}
                held={moving && held}
                paused={key === pausedAt}
              />
            );
          })}
        </div>
      </TooltipTrigger>
      <TooltipContent>{hintOf(item, s, embedStage)}</TooltipContent>
    </Tooltip>
  );
}

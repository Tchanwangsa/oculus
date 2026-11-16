import { memo } from "react";
import { CircleNotch, Warning } from "@phosphor-icons/react";
import { fmtClock, sqliteUtcToMs } from "@/lib/format/format";
import { steerMeta, steerStatus, type HarnessItem } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { QuestionBubble } from "./QuestionBubble";

/**
 * A message the student wrote into the running run (a steer), at its place
 * among the run's work. It reads as their question does, with no Edit or
 * Rewind: it is part of a run, not one of its own. Until the provider echoes
 * it a line under the bubble says how far it got; a write that broke says it
 * may not have arrived, since it is never sent again.
 */
export const SteerRow = memo(function SteerRow({ item }: { item: HarnessItem }) {
  const status = steerStatus(steerMeta(item).delivery);
  return (
    <QuestionBubble
      text={item.content ?? ""}
      when={fmtClock(sqliteUtcToMs(item.created_at))}
      status={status && (
        <div
          role="status"
          className={cn(
            "mt-1 flex items-center justify-end gap-1 px-1 text-[11px] leading-5",
            status.tone === "warning" ? "text-warning" : "text-muted-foreground",
          )}
        >
          {status.busy ? (
            <CircleNotch size={11} weight="bold" className="shrink-0 animate-spin text-brand" aria-hidden />
          ) : (
            <Warning size={11} weight="bold" className="shrink-0" aria-hidden />
          )}
          <span className="min-w-0 text-right">{status.text}</span>
        </div>
      )}
    />
  );
});

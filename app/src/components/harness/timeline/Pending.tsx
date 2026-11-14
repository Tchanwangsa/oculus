import { useShallow } from "zustand/react/shallow";
import { ANCHOR_ROW } from "@/hooks/ui/useStickToBottom";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { cn } from "@/lib/utils";
import { LiveThinkingRow } from "./WorkRow";
import { Working } from "./Working";
import { runningSince } from "@/lib/harness";
import { Assistant } from "./MessageActions";
import { QuestionBubble } from "./QuestionBubble";
import type { PendingActions } from "./types";

/** The turn in flight: reasoning and text with no row yet, or a steady
 *  "Working for 8s" (`quiet`; off while the run's fold shows its own clock).
 *  One anchor row, never `skip-offscreen`: it changes size every fold and is
 *  on screen whenever it is read. `text` off: the streaming reply holds a
 *  widget and `Timeline` draws it as the row it will become. */
export function LiveTail({
  threadId,
  quiet: allowQuiet = true,
  text = true,
}: {
  threadId: number;
  quiet?: boolean;
  text?: boolean;
}) {
  const { running, streaming, thinking } = useHarnessStore(useShallow((s) => ({
    running: s.live[threadId]?.running ?? false,
    streaming: s.live[threadId]?.streaming ?? "",
    thinking: s.live[threadId]?.thinking ?? "",
  })));
  // The run in flight's start; none for a turn the agent started itself.
  const since = useHarnessStore((s) => runningSince(s.runs[threadId]));
  const quiet = allowQuiet && running && !streaming && !thinking;
  const shown = text ? streaming : "";
  if (!thinking && !shown && !quiet) return null;
  return (
    <div className={cn(ANCHOR_ROW, "flex flex-col gap-2")}>
      {thinking && <LiveThinkingRow text={thinking} />}
      {shown && <Assistant text={shown} />}
      {quiet && (
        <div className="mt-1 px-2 text-xs leading-5 tabular-nums text-muted-foreground">
          <Working since={since} />
        </div>
      )}
    </div>
  );
}

/** Messages queued while the agent works — held in memory by Rust (`Queue` in
 *  `app/src-tauri/src/harness/mod.rs`), editable or removable until sent. */
export function Pending({ threadId, actions }: { threadId: number; actions: PendingActions }) {
  const queued = useHarnessStore((s) => s.queued[threadId]);
  if (!queued?.length) return null;
  return (
    <>
      {queued.map((q) => (
        <div key={q.id} className={ANCHOR_ROW}>
          <QuestionBubble
            text={q.text}
            pending
            onSubmit={(text) => actions.editQueued(q.id, text)}
            onRemove={() => actions.unqueue(q.id)}
          />
        </div>
      ))}
    </>
  );
}

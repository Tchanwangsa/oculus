import { CircleNotch } from "@phosphor-icons/react";
import { useShallow } from "zustand/react/shallow";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { ThinkingRow } from "./WorkRow";
import { Assistant } from "./MessageActions";
import { QuestionBubble } from "./QuestionBubble";
import type { PendingActions } from "./types";

/** The turn in flight: reasoning and text with no row yet, or "Working…". */
export function LiveTail({ threadId }: { threadId: number }) {
  const { running, streaming, thinking } = useHarnessStore(useShallow((s) => ({
    running: s.live[threadId]?.running ?? false,
    streaming: s.live[threadId]?.streaming ?? "",
    thinking: s.live[threadId]?.thinking ?? "",
  })));
  const quiet = running && !streaming && !thinking;
  return (
    <>
      {thinking && <ThinkingRow text={thinking} live />}
      {streaming && <Assistant text={streaming} />}
      {quiet && (
        <div className="mt-1 flex items-center gap-2 px-2 text-xs text-muted-foreground">
          <CircleNotch size={13} className="animate-spin" />
          <span className="animate-pulse">Working…</span>
        </div>
      )}
    </>
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
        <QuestionBubble
          key={q.id}
          text={q.text}
          pending
          onSubmit={(text) => actions.editQueued(q.id, text)}
          onRemove={() => actions.unqueue(q.id)}
        />
      ))}
    </>
  );
}

import { memo } from "react";
import { ArrowClockwise } from "@phosphor-icons/react";
import { fmtClock, sqliteUtcToMs } from "@/lib/format/format";
import { messageAt, type HarnessItem } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { Action, Assistant, CopyAction, MessageActions } from "./MessageActions";
import { QuestionBubble } from "./QuestionBubble";
import type { QuestionActions } from "./types";

/** Busy is read from the store here, not passed in: it changes twice a turn,
 *  and a prop would re-render every committed row. */
export const User = memo(function User({
  item,
  actions,
}: {
  item: HarnessItem;
  actions?: QuestionActions;
}) {
  const busy = useHarnessStore((s) => s.live[item.thread_id]?.running ?? false);
  const text = item.content ?? "";
  const edit = busy ? undefined : actions?.edit;
  const rewind = busy ? undefined : actions?.rewind;
  return (
    <QuestionBubble
      msgId={item.id}
      text={text}
      when={fmtClock(sqliteUtcToMs(item.created_at))}
      at={messageAt(item)}
      onSubmit={edit ? (next) => edit(item.id, next) : undefined}
      onRewind={rewind ? () => rewind(item.id) : undefined}
    />
  );
});

/** An answer, with Copy and Retry under it. */
export const Reply = memo(function Reply({
  item,
  asked,
  actions,
}: {
  item: HarnessItem;
  /** The question this answered, for Retry. */
  asked?: HarnessItem;
  actions?: QuestionActions;
}) {
  const busy = useHarnessStore((s) => s.live[item.thread_id]?.running ?? false);
  const text = item.content ?? "";
  const retry = actions?.retry;
  return (
    <div className="group/msg flex w-full min-w-0 flex-col">
      <Assistant text={text} />
      <MessageActions when={fmtClock(sqliteUtcToMs(item.created_at))} side="left">
        <CopyAction text={text} />
        {retry && asked && !busy && (
          <Action
            label="Retry"
            icon={ArrowClockwise}
            onClick={() => retry(asked.id, asked.content ?? "")}
          />
        )}
      </MessageActions>
    </div>
  );
});

/** Where a turn was stopped. */
export const Stopped = memo(function Stopped() {
  return (
    <div className="py-1 text-center text-[11px] text-muted-foreground">You stopped the response</div>
  );
});

/** A rewind that took rows off the screen but not out of the agent's context
 *  (no anchor, or a dropped CLI session) — shown so the mismatch is visible. */
export const ContextDrift = memo(function ContextDrift({ threadId }: { threadId: number }) {
  const drifted = useHarnessStore((s) => s.contextDrift[threadId] ?? false);
  if (!drifted) return null;
  return (
    <div className="py-1 text-center text-[11px] text-muted-foreground">
      The agent still remembers what was removed here
    </div>
  );
});

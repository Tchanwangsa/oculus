import { memo } from "react";
import { ArrowClockwise } from "@phosphor-icons/react";
import { fmtClock, sqliteUtcToMs } from "@/lib/format/format";
import { messageAt, unpromptedAfter, type HarnessItem } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { Action, Assistant, CopyAction, MessageActions } from "./MessageActions";
import { QuestionBubble } from "./QuestionBubble";
import type { QuestionActions } from "./types";
import type { ReplyRole } from "./rows";
import { mayHoldWidget } from "@/components/markdown/openui/remarkWidgets";
import { WidgetRow } from "@/components/markdown/openui/state";

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

/** An answer, with Copy and Retry under it. A run still working shows its
 *  latest reply without them; a stopped or failed run's says it is partial.
 *  Copy takes this reply alone, never the folded work above it. */
export const Reply = memo(function Reply({
  item,
  asked,
  actions,
  role,
}: {
  item: HarnessItem;
  /** The question this answered, for Retry. */
  asked?: HarnessItem;
  actions?: QuestionActions;
  role?: ReplyRole;
}) {
  const busy = useHarnessStore((s) => s.live[item.thread_id]?.running ?? false);
  const text = item.content ?? "";
  const retry = actions?.retry;
  const settled = role !== "provisional";
  return (
    <div data-revision-row={item.id} className="group/msg flex w-full min-w-0 flex-col">
      {role === "partial" && <div className="px-2 pb-1 text-[11px] text-muted-foreground">Partial response</div>}
      <WidgetRow.Provider value={{ thread: item.thread_id, row: item.id }}>
        <Assistant text={text} />
      </WidgetRow.Provider>
      <MessageActions when={fmtClock(sqliteUtcToMs(item.created_at))} side="left">
        {settled && <CopyAction text={text} />}
        {settled && retry && asked && !busy && (
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

/** An earlier reply inside its run's fold: its prose, with a pointer where
 *  each widget fence was (they draw once, in `ArtifactRow`). */
export const FoldedReply = memo(function FoldedReply({ item }: { item: HarnessItem }) {
  const text = item.content ?? "";
  if (!text.trim()) return null;
  return <Assistant text={text} split={mayHoldWidget(text) ? "prose" : undefined} />;
});

/** The widget fences of a folded reply, outside the fold so a card, a proposal
 *  and its Undo, or a widget, mounts once whether the fold is open or not.
 *  Keyed as the whole reply would be: its row id and original fence index. */
export const ArtifactRow = memo(function ArtifactRow({ item }: { item: HarnessItem }) {
  return (
    <WidgetRow.Provider value={{ thread: item.thread_id, row: item.id }}>
      <Assistant text={item.content ?? ""} split="artifacts" />
    </WidgetRow.Provider>
  );
});

/** Where a turn was stopped. */
export const Stopped = memo(function Stopped() {
  return (
    <div className="py-1 text-center text-[11px] text-muted-foreground">You stopped the response</div>
  );
});

/** A run whose answer, or part of it, never arrived (Rust's `DeliveryNotice`).
 *  Nothing is resent; the student can ask again. */
export const DeliveryNotice = memo(function DeliveryNotice({ item }: { item: HarnessItem }) {
  return <div className="py-1 text-center text-[11px] text-muted-foreground">{item.content}</div>;
});

/** The agent compacted its context here; its summary is not shown as a reply. */
export const Compacted = memo(function Compacted() {
  return <div className="py-1 text-center text-[11px] text-muted-foreground">Context compacted</div>;
});

/** Where the agent went on without a message: a background task finished. */
export const Unprompted = memo(function Unprompted({ item }: { item: HarnessItem }) {
  const names = unpromptedAfter(item).map((n) => `“${n}”`);
  const list =
    names.length > 1 ? `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}` : names[0];
  return (
    <div className="py-1 text-center text-[11px] text-muted-foreground">
      {list ? `Continued after ${list} finished` : "Continued on its own"}
    </div>
  );
});

/** Heads a card's continuation run: Oculus's line about the card that
 *  settled, never drawn as the student's message. */
export const Continued = memo(function Continued({ item }: { item: HarnessItem }) {
  return <div className="py-1 text-center text-[11px] text-muted-foreground">{item.content}</div>;
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

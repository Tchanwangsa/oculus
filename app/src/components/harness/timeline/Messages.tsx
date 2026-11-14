import { memo, useMemo } from "react";
import {
  ArrowBendDownRight,
  ArrowClockwise,
  ArrowsInSimple,
  ClockCounterClockwise,
  StopCircle,
  WarningCircle,
} from "@phosphor-icons/react";
import { fmtClock, sqliteUtcToMs } from "@/lib/format/format";
import { messageAt, unpromptedAfter, type HarnessItem } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { Action, Assistant, CopyAction, MessageActions } from "./MessageActions";
import { QuestionBubble } from "./QuestionBubble";
import { SystemDivider } from "./SystemDivider";
import type { QuestionActions } from "./types";
import type { ReplyRole } from "./rows";
import { mayHoldWidget } from "@/components/markdown/openui/remarkWidgets";
import { WidgetRow } from "@/components/markdown/openui/state";
import { WidgetStream } from "@/components/markdown/widget/stream/context";

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
 *  Copy takes this reply alone, never the folded work above it.
 *
 *  `stream`: the reply still arriving, read from the store as it streams.
 *  It draws through the same elements as the committed reply, which takes
 *  its row key, so what it holds (a widget's frame) carries over; its
 *  fences see no `WidgetRow` until then, only `WidgetStream`. */
export const Reply = memo(function Reply({
  item,
  asked,
  actions,
  role,
  stream = false,
}: {
  item: HarnessItem;
  /** The question this answered, for Retry. */
  asked?: HarnessItem;
  actions?: QuestionActions;
  role?: ReplyRole;
  stream?: boolean;
}) {
  const busy = useHarnessStore((s) => s.live[item.thread_id]?.running ?? false);
  // Null for every committed reply, so none re-renders per token.
  const streamed = useHarnessStore((s) => (stream ? (s.live[item.thread_id]?.streaming ?? "") : null));
  const text = streamed ?? item.content ?? "";
  const retry = actions?.retry;
  const settled = role !== "provisional";
  const widgetRow = useMemo(() => (stream ? null : { thread: item.thread_id, row: item.id }), [stream, item.thread_id, item.id]);
  const widgetStream = useMemo(
    () => (stream && item.run_id != null ? { thread: item.thread_id, run: item.run_id } : null),
    [stream, item.thread_id, item.run_id],
  );
  return (
    <div data-revision-row={stream ? undefined : item.id} className="group/msg flex w-full min-w-0 flex-col">
      {role === "partial" && <div className="px-2 pb-1 text-[11px] text-muted-foreground">Partial response</div>}
      <WidgetRow.Provider value={widgetRow}>
        <WidgetStream.Provider value={widgetStream}>
          <Assistant text={text} />
        </WidgetStream.Provider>
      </WidgetRow.Provider>
      {!stream && (
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
      )}
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
  return <SystemDivider icon={StopCircle} label="Run interrupted" detail="Stopped by you" />;
});

/** A run whose answer, or part of it, never arrived (Rust's `DeliveryNotice`).
 *  Nothing is resent; the student can ask again. */
export const DeliveryNotice = memo(function DeliveryNotice({ item }: { item: HarnessItem }) {
  return <SystemDivider icon={WarningCircle} label={item.content ?? ""} />;
});

/** The agent compacted its context here; its summary is not shown as a reply. */
export const Compacted = memo(function Compacted() {
  return <SystemDivider icon={ArrowsInSimple} label="Context compacted" />;
});

/** Where the agent went on without a message: a background task finished. */
export const Unprompted = memo(function Unprompted({ item }: { item: HarnessItem }) {
  const names = unpromptedAfter(item).map((n) => `“${n}”`);
  const list =
    names.length > 1 ? `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}` : names[0];
  return (
    <SystemDivider icon={ArrowBendDownRight} label={list ? `Continued after ${list} finished` : "Continued on its own"} />
  );
});

/** Heads a card's continuation run: Oculus's line about the card that
 *  settled, never drawn as the student's message. */
export const Continued = memo(function Continued({ item }: { item: HarnessItem }) {
  return <SystemDivider icon={ArrowBendDownRight} label={item.content ?? ""} />;
});

/** A rewind that took rows off the screen but not out of the agent's context
 *  (no anchor, or a dropped CLI session) — shown so the mismatch is visible. */
export const ContextDrift = memo(function ContextDrift({ threadId }: { threadId: number }) {
  const drifted = useHarnessStore((s) => s.contextDrift[threadId] ?? false);
  if (!drifted) return null;
  return <SystemDivider icon={ClockCounterClockwise} label="The agent still remembers what was removed here" />;
});

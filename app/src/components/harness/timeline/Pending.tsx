import { useEffect, useState } from "react";
import { CaretDown, CaretUp, Clock, Pause } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { useShallow } from "zustand/react/shallow";
import { ANCHOR_ROW } from "@/hooks/ui/useStickToBottom";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { cn } from "@/lib/utils";
import { LiveThinkingRow } from "./WorkRow";
import { Working } from "./Working";
import { commandError, moveQueued, queuedLabel, runningSince, type QueuedMessage } from "@/lib/harness";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useRunCommand } from "@/components/harness/composer/useRunCommand";
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

/** Messages waiting their turn: queued runs Rust saves
 *  (`harness/store/queue.rs`), editable, removable and movable until sent.
 *  Each says above its bubble whether it is held or only waiting: after a
 *  stop, a failed turn or a restart they stay listed, held, and go out on
 *  Resume or with the next message. A command that fails says why under
 *  the list. */
export function Pending({ threadId, actions }: { threadId: number; actions: PendingActions }) {
  const queued = useHarnessStore((s) => s.queued[threadId]);
  if (!queued?.length) return null;
  return <PendingList threadId={threadId} queued={queued} actions={actions} />;
}

/** `Pending`'s list, given the messages. */
export function PendingList({
  threadId,
  queued,
  actions,
}: {
  threadId: number;
  queued: QueuedMessage[];
  actions: PendingActions;
}) {
  // A reorder is one at a time (the arrows rest while it is out); an edit
  // or a removal goes at once, and the newest failure is the one shown.
  const reorder = useRunCommand();
  const [error, setError] = useState<string | null>(null);
  const { clear } = reorder;
  useEffect(() => {
    clear();
    setError(null);
  }, [threadId, clear]);
  const ids = queued.map((q) => q.id);
  const move = (id: string, by: -1 | 1) => {
    const order = moveQueued(ids, id, by);
    if (!order) return;
    setError(null);
    void reorder.run(() => actions.reorder(order));
  };
  const act = (what: string, command: () => Promise<void>) => {
    reorder.clear();
    setError(null);
    command().catch((e) => setError(`${what}: ${commandError(e)}`));
  };
  const shown = error ?? (reorder.error ? `Couldn't reorder: ${reorder.error}` : null);
  return (
    <>
      {queued.map((q, i) => (
        <div key={q.id} className={ANCHOR_ROW}>
          <QueuedState
            held={q.held}
            earlier={queued.length > 1 ? () => move(q.id, -1) : undefined}
            later={queued.length > 1 ? () => move(q.id, 1) : undefined}
            first={i === 0}
            last={i === queued.length - 1}
            busy={reorder.pending}
          />
          <QuestionBubble
            text={q.text}
            pending
            onSubmit={(text) => act("Couldn't save the edit", () => actions.editQueued(q.id, text))}
            onRemove={() => act("Couldn't remove it", () => actions.unqueue(q.id))}
          />
          {/* Inside the last row, so the timeline's anchoring sees one row per message. */}
          {shown && i === queued.length - 1 && (
            <div className="px-2 text-right text-[11px] text-destructive">{shown}</div>
          )}
        </div>
      ))}
    </>
  );
}

/** The line above a waiting message: its order controls when there is more
 *  than one, and its state in words. Always shown, so nothing fades. */
function QueuedState({
  held,
  earlier,
  later,
  first,
  last,
  busy,
}: {
  held: boolean;
  earlier?: () => void;
  later?: () => void;
  first: boolean;
  last: boolean;
  busy: boolean;
}) {
  const StateIcon = held ? Pause : Clock;
  return (
    <div className="flex h-6 items-center justify-end gap-0.5 text-[11px] text-muted-foreground">
      {earlier && <MoveButton label="Move earlier" icon={CaretUp} disabled={first || busy} onClick={earlier} />}
      {later && <MoveButton label="Move later" icon={CaretDown} disabled={last || busy} onClick={later} />}
      <span className="inline-flex items-center gap-1 px-1.5">
        <StateIcon size={11} weight={held ? "fill" : "regular"} aria-hidden />
        {queuedLabel({ held })}
      </span>
    </div>
  );
}

function MoveButton({ label, icon: IconC, disabled, onClick }: {
  label: string;
  icon: Icon;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button size="icon-xs" variant="ghost" aria-label={label} disabled={disabled} onClick={onClick}>
          <IconC />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

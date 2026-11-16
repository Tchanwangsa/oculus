import { useEffect, useState } from "react";
import { ArrowUp, CaretDown, CaretUp, Clock, Pause } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { useShallow } from "zustand/react/shallow";
import { ANCHOR_ROW } from "@/hooks/ui/useStickToBottom";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { cn } from "@/lib/utils";
import { LiveThinkingRow } from "./WorkRow";
import { Working } from "./Working";
import {
  commandError,
  isSlashCommand,
  moveQueued,
  queuedLabel,
  runningSince,
  runWaitingOn,
  SEND_NOW,
  steerHint,
  WAITING_LABEL,
  type QueuedMessage,
  type SteerKind,
} from "@/lib/harness";
import { useSteerKind } from "@/hooks/agents/useSteerKind";
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
  const waiting = useHarnessStore((s) => runWaitingOn(s.requests[threadId]));
  const quiet = allowQuiet && running && !streaming && !thinking;
  const shown = text ? streaming : "";
  if (!thinking && !shown && !quiet) return null;
  return (
    <div className={cn(ANCHOR_ROW, "flex flex-col gap-2")}>
      {thinking && <LiveThinkingRow text={thinking} />}
      {shown && <Assistant text={shown} />}
      {quiet && (
        <div className="mt-1 px-2 text-xs leading-5 tabular-nums text-muted-foreground">
          {waiting ? WAITING_LABEL[waiting] : <Working since={since} />}
        </div>
      )}
    </div>
  );
}

/** Messages waiting their turn: queued runs Rust saves
 *  (`harness/store/queue.rs`), editable, removable and movable until sent.
 *  Each says above its bubble whether it is held or only waiting: after a
 *  stop, a failed turn or a restart they stay listed, held, and go out on
 *  Resume or with the next message. While the running turn can take a
 *  message, each that is not a command offers "Send now". A command that
 *  fails says why under the list. */
export function Pending({ threadId, actions }: { threadId: number; actions: PendingActions }) {
  const queued = useHarnessStore((s) => s.queued[threadId]);
  const steer = useSteerKind(threadId);
  if (!queued?.length) return null;
  return <PendingList threadId={threadId} queued={queued} actions={actions} steer={steer} />;
}

/** `Pending`'s list, given the messages. */
export function PendingList({
  threadId,
  queued,
  actions,
  steer = null,
}: {
  threadId: number;
  queued: QueuedMessage[];
  actions: PendingActions;
  /** How the running turn takes a message; null offers no "Send now". */
  steer?: SteerKind;
}) {
  // A reorder is one at a time (the arrows rest while it is out); an edit
  // or a removal goes at once, and the newest failure is the one shown.
  const reorder = useRunCommand();
  // One "Send now" at a time: the buttons rest while it is out.
  const sendNow = useRunCommand();
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
  const now = (id: string) => {
    reorder.clear();
    setError(null);
    void sendNow.run(() => actions.steerQueued(id));
  };
  const shown = error ??
    (reorder.error ? `Couldn't reorder: ${reorder.error}` : sendNow.error ? `Couldn't send it now: ${sendNow.error}` : null);
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
            sendNow={steer && !isSlashCommand(q.text) ? { hint: steerHint(steer), busy: sendNow.pending, onClick: () => now(q.id) } : undefined}
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

/** The line above a waiting message: "Send now" where the running turn can
 *  take it, its order controls when there is more than one, and its state in
 *  words. Always shown, so nothing fades. */
function QueuedState({
  held,
  earlier,
  later,
  first,
  last,
  busy,
  sendNow,
}: {
  held: boolean;
  earlier?: () => void;
  later?: () => void;
  first: boolean;
  last: boolean;
  busy: boolean;
  sendNow?: { hint: string; busy: boolean; onClick: () => void };
}) {
  const StateIcon = held ? Pause : Clock;
  return (
    <div className="flex h-6 items-center justify-end gap-0.5 text-[11px] text-muted-foreground">
      {sendNow && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              size="xs"
              variant="ghost"
              disabled={sendNow.busy}
              onClick={sendNow.onClick}
              aria-description={sendNow.hint}
              className="will-change-[opacity]"
            >
              <ArrowUp aria-hidden />
              {SEND_NOW}
            </Button>
          </TooltipTrigger>
          <TooltipContent>{sendNow.hint}</TooltipContent>
        </Tooltip>
      )}
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
        <Button
          size="icon-xs"
          variant="ghost"
          aria-label={label}
          disabled={disabled}
          onClick={onClick}
          // Disabled fades its icon; its own layer keeps it from jittering.
          className="will-change-[opacity]"
        >
          <IconC />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

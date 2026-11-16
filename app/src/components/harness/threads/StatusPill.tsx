import { useEffect, useState } from "react";
import { CheckCircle, Question, ShieldWarning, WarningCircle } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import {
  elapsedText,
  THREAD_STATUS_LABEL,
  threadStatus,
  threadUnread,
  type HarnessThread,
  type ThreadStatus,
} from "@/lib/harness";
import { sqliteUtcToMs } from "@/lib/format/format";
import { cn } from "@/lib/utils";

/** Each status's glyph (none for Working, which draws a dashed ring), its
 *  text tone and the folded column's dot. Brand means Working only; what
 *  waits on the student (Approval, Question) is the warning yellow. */
export const STATUS_LOOK: Record<ThreadStatus, { icon: Icon | null; text: string; dot: string }> = {
  working: { icon: null, text: "text-brand", dot: "bg-brand" },
  approval: { icon: ShieldWarning, text: "text-warning", dot: "bg-warning" },
  input: { icon: Question, text: "text-warning", dot: "bg-warning" },
  failed: { icon: WarningCircle, text: "text-destructive", dot: "bg-destructive" },
  done: { icon: CheckCircle, text: "text-success", dot: "bg-success" },
};

/** What a thread that ended, waits or failed since it was last on screen
 *  shows: a bright, medium-weight title. */
export const ATTENTION: ReadonlySet<ThreadStatus> = new Set(["done", "approval", "input", "failed"]);

/** A listed thread's status. `running` comes from the page's running ids,
 *  not the store's live map, which changes on every streamed token.
 *  Approval is the row's `pending_requests`, so a thread never opened shows
 *  it too. Questions count separately for Question. */
export function threadRowStatus(thread: HarnessThread, running: boolean): ThreadStatus | null {
  return threadStatus({
    running,
    failed: thread.status === "error",
    unread: threadUnread(thread),
    approval: (thread.pending_requests ?? 0) > (thread.pending_questions ?? 0),
    input: (thread.pending_questions ?? 0) > 0,
  });
}

/** When the thread's run in flight started, for the Working timer. */
export const runStartedMs = (thread: HarnessThread): number | null =>
  sqliteUtcToMs(thread.run_started_at ?? null) ?? null;

/** T3's working mark: a dashed ring turning slowly, still under reduced motion. */
export function WorkingRing({ className }: { className?: string }) {
  return (
    <span
      aria-hidden
      className={cn(
        "inline-block size-3 shrink-0 rounded-full border-[1.5px] border-dashed border-current motion-safe:animate-[spin_4s_linear_infinite]",
        className,
      )}
    />
  );
}

/** "9s", "1m 4s", ticking. Its own component, so the second hand re-renders
 *  this text and not the row or the list. */
export function Elapsed({ since }: { since: number }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [since]);
  return <span className="tabular-nums">{elapsedText(Math.max(0, now - since))}</span>;
}

/** The status as a glyph and a word, in its tone; Working adds its timer
 *  when `since` (the run's start) is known. */
export function StatusPill({
  status,
  since,
  className,
}: {
  status: ThreadStatus;
  since?: number | null;
  className?: string;
}) {
  const look = STATUS_LOOK[status];
  const IconC = look.icon;
  return (
    <span className={cn("inline-flex h-5 shrink-0 items-center gap-1 text-[11.5px] font-medium", look.text, className)}>
      {IconC ? <IconC size={12} weight="bold" className="shrink-0" aria-hidden /> : <WorkingRing />}
      {THREAD_STATUS_LABEL[status]}
      {status === "working" && since != null && <Elapsed since={since} />}
    </span>
  );
}

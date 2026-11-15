import { CheckCircle, CircleNotch, Question, ShieldWarning, WarningCircle } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import {
  awaitsApproval,
  THREAD_STATUS_LABEL,
  threadStatus,
  threadUnread,
  type HarnessThread,
  type ThreadStatus,
} from "@/lib/harness";
import { cn } from "@/lib/utils";
import { useHarnessStore } from "@/stores/chat/harnessStore";

/** Each status's glyph, its text tone and the folded rail dot's fill. Brand
 *  is for what is in flight or waiting on the student; never info blue. */
export const STATUS_LOOK: Record<ThreadStatus, { icon: Icon; text: string; dot: string; spin?: boolean }> = {
  working: { icon: CircleNotch, text: "text-brand", dot: "bg-brand", spin: true },
  approval: { icon: ShieldWarning, text: "text-warning", dot: "bg-warning" },
  input: { icon: Question, text: "text-brand", dot: "bg-brand" },
  failed: { icon: WarningCircle, text: "text-destructive", dot: "bg-destructive" },
  done: { icon: CheckCircle, text: "text-success", dot: "bg-success" },
};

/** A listed thread's status. `running` comes from the page's running ids,
 *  not the store's live map, which changes on every streamed token.
 *  `approval`: a request of the thread waits on the student. No bridge
 *  asks questions yet, so input is never set. */
export function threadRowStatus(thread: HarnessThread, running: boolean, approval = false): ThreadStatus | null {
  return threadStatus({
    running,
    failed: thread.status === "error",
    unread: threadUnread(thread),
    approval,
    input: false,
  });
}

/** `threadRowStatus`, with Approval read from the thread's requests in the
 *  store. Only a thread whose rows were loaded has them: Rust's thread list
 *  carries no "a request waits" flag yet, so an unloaded thread never reads
 *  Approval. */
export function useThreadRowStatus(thread: HarnessThread, running: boolean): ThreadStatus | null {
  const approval = useHarnessStore((s) => awaitsApproval(s.requests[thread.id]));
  return threadRowStatus(thread, running, approval);
}

/** The status as an icon and a word, in its tone. */
export function StatusPill({ status, className }: { status: ThreadStatus; className?: string }) {
  const look = STATUS_LOOK[status];
  const IconC = look.icon;
  return (
    <span className={cn("inline-flex h-5 shrink-0 items-center gap-1 text-[11px] font-medium", look.text, className)}>
      <IconC size={12} weight={look.spin ? "bold" : "fill"} className={cn("shrink-0", look.spin && "animate-spin")} aria-hidden />
      {THREAD_STATUS_LABEL[status]}
    </span>
  );
}

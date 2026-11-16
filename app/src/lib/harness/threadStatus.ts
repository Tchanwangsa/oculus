import { sqliteUtcToMs } from "@/lib/format/format";

/** A thread's status as the sidebar pill and the announcer name it. */
export type ThreadStatus = "approval" | "input" | "working" | "failed" | "done";

export interface ThreadStatusInputs {
  /** A run is in flight (`harnessStore.live[id].running`). */
  running: boolean;
  /** Its last run failed (`harness_threads.status === "error"`). */
  failed: boolean;
  /** A run settled after the student last had the thread on screen. */
  unread: boolean;
  /** A tool waits on the student's approval. */
  approval: boolean;
  /** The agent asked the student a question. */
  input: boolean;
}

/**
 * The one status a thread shows, first match wins: Approval, Input, Working,
 * Failed, Done. What needs the student comes before what they can only watch,
 * and a failure outranks an unread success. Null is a quiet thread.
 */
export function threadStatus(i: ThreadStatusInputs): ThreadStatus | null {
  if (i.approval) return "approval";
  if (i.input) return "input";
  if (i.running) return "working";
  if (i.failed) return "failed";
  if (i.unread) return "done";
  return null;
}

export const THREAD_STATUS_LABEL: Record<ThreadStatus, string> = {
  approval: "Approval",
  input: "Question",
  working: "Working",
  failed: "Failed",
  done: "Done",
};

/** A run ended after the student last had the thread on screen. A thread
 *  never viewed is unread once any run has ended. */
export function threadUnread(t: { viewed_at?: string | null; settled_at?: string | null }): boolean {
  const settled = sqliteUtcToMs(t.settled_at ?? null);
  if (settled == null) return false;
  const viewed = sqliteUtcToMs(t.viewed_at ?? null);
  return viewed == null || settled > viewed;
}

import { QUEUE_LABEL, STEER_LABEL, steerHint, type SendAction } from "./sendMode";
import type { HarnessRun, QueuedMessage } from "./types";

/** What the composer's Stop button is: its words, whether it takes a click,
 *  and the run it names (null for a turn the CLI started itself). */
export interface StopControl {
  label: "Stop generation" | "Stopping";
  stopping: boolean;
  runId: number | null;
}

/** What the composer's main button does: start, queue or steer the draft,
 *  or lift the hold on a queue (`resume`). `hint` is its tooltip where the
 *  label alone does not say what happens (a steer). */
export interface SendControl {
  kind: "send" | "queue" | "steer" | "resume";
  label: string;
  disabled: boolean;
  hint?: string;
}

export interface RunControlInputs {
  /** A turn is open on the chat (`harnessStore.live[id].running`). */
  running: boolean;
  /** The run on screen (`liveRunId`), or null for a turn the CLI started. */
  liveRun: Pick<HarnessRun, "id" | "stopping"> | null;
  /** A stop is on its way, or reached a turn that has no run to mark. */
  stopSent: boolean;
  /** Some waiting message is held (`queueHeld`). */
  held: boolean;
  /** The box holds words, pictures or pasted cards. */
  hasDraft: boolean;
  /** `canSend`: there is a draft and the chat's scope is chosen. */
  ready: boolean;
  /** Attachments are still being written to the library. */
  writing: boolean;
  /** What the draft would do into an open turn (`sendAction`); a queue
   *  when not given. */
  action?: SendAction;
}

export const RESUME = "Resume";
export const RESUME_THREAD = "Resume thread";

/** The composer's Resume, as T3 words it: with the box empty it is the main
 *  button and resumes the thread; elsewhere (the held notice) plain Resume. */
export function resumeLabel(draftEmpty: boolean): string {
  return draftEmpty ? RESUME_THREAD : RESUME;
}

/**
 * The composer's Stop and main button. Stop shows while a turn is open and
 * reads "Stopping", inert, once the run is marked or a stop is on its way.
 * The main button queues or steers into an open turn, as `action` says,
 * resumes a held queue when the box is empty, and otherwise sends; a draft
 * sent past a held queue lifts the hold and goes out after the waiting
 * messages (`docs/harness.md`).
 */
export function runControls(i: RunControlInputs): { stop: StopControl | null; send: SendControl | null } {
  if (i.running) {
    const stopping = i.stopSent || !!i.liveRun?.stopping;
    return {
      stop: { label: stopping ? "Stopping" : "Stop generation", stopping, runId: i.liveRun?.id ?? null },
      // Nothing to send: the box keeps only Stop, as it always has.
      send: i.ready ? intoTurn(i.action ?? { kind: "queue" }, i.writing) : null,
    };
  }
  if (i.held && !i.hasDraft) {
    return { stop: null, send: { kind: "resume", label: resumeLabel(true), disabled: false } };
  }
  return {
    stop: null,
    send: {
      kind: "send",
      label: i.held ? "Send after the waiting messages" : "Send message",
      disabled: !i.ready || i.writing,
    },
  };
}

function intoTurn(action: SendAction, writing: boolean): SendControl {
  if (action.kind !== "steer") return { kind: "queue", label: QUEUE_LABEL, disabled: writing };
  return { kind: "steer", label: STEER_LABEL, hint: steerHint(action.restart ? "restart" : "native"), disabled: writing };
}

/** Whether any of the chat's waiting messages is held. */
export function queueHeld(queued: QueuedMessage[] | undefined): boolean {
  return !!queued?.some((q) => q.held);
}

/** A waiting message's state, in words beside its bubble. */
export function queuedLabel(q: Pick<QueuedMessage, "held">): "Held" | "Waiting" {
  return q.held ? "Held" : "Waiting";
}

/** The waiting messages' ids with `id` moved `by` places (-1 earlier, +1
 *  later); null when it cannot move that way. */
export function moveQueued(ids: string[], id: string, by: -1 | 1): string[] | null {
  const from = ids.indexOf(id);
  const to = from + by;
  if (from < 0 || to < 0 || to >= ids.length) return null;
  const next = [...ids];
  [next[from], next[to]] = [next[to], next[from]];
  return next;
}

/** Force stop's words: what it will cost is said on the button. `others` is
 *  how many other chats have a run on the same server (`stop_unresponsive`);
 *  it only counts where the bridge shares one process between chats. */
export function forceStopLabel(others: number, sharedProcess: boolean): string {
  if (!sharedProcess || others <= 0) return "Force stop";
  return `Force stop — also stops ${others} other ${others === 1 ? "chat" : "chats"}`;
}

/** A failed command's reason, for the line beside the control the student
 *  used. Tauri rejects with the Rust error's string. */
export function commandError(e: unknown): string {
  const text = e instanceof Error ? e.message : typeof e === "string" ? e : "";
  return text.trim() || "Something went wrong";
}

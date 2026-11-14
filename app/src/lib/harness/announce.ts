import { threadStatus, type ThreadStatus } from "./threadStatus";
import type { RunStatus } from "./types";

/** What the announcer can say about the open thread. */
export type Spoken = ThreadStatus | "stopped";

export const SPOKEN_WORDS: Record<Spoken, string> = {
  working: "Working",
  approval: "Needs approval",
  input: "Needs your input",
  done: "Done",
  failed: "Failed",
  stopped: "Stopped",
};

/** The open thread's status as spoken: the sidebar's order, with its last
 *  run's ending standing in for unread, and a stop said as such. */
export function spokenStatus(i: {
  running: boolean;
  approval: boolean;
  input: boolean;
  lastRun: RunStatus | null;
}): Spoken | null {
  const s = threadStatus({
    running: i.running,
    approval: i.approval,
    input: i.input,
    failed: i.lastRun === "failed",
    unread: i.lastRun === "completed",
  });
  if (s) return s;
  return i.lastRun === "interrupted" ? "stopped" : null;
}

export interface Timers {
  set: (fn: () => void, ms: number) => unknown;
  clear: (handle: unknown) => void;
}

const REAL: Timers = {
  set: (fn, ms) => setTimeout(fn, ms),
  clear: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
};

/** How long a status must hold before it is said: a queued message going out
 *  flips Done to Working within a few milliseconds. */
export const SETTLE_MS = 300;

/**
 * Says the open thread's status when it changes, through `say`: never the
 * status it starts with (a mount or a thread switch, via `reset`), only one
 * that holds for `SETTLE_MS`, and never the same word twice in a row.
 */
export function createAnnouncer(say: (text: string) => void, timers: Timers = REAL) {
  let primed = false;
  let spoken: Spoken | null = null;
  let timer: unknown = null;
  const cancel = () => {
    if (timer != null) timers.clear(timer);
    timer = null;
  };
  return {
    update(next: Spoken | null) {
      if (!primed) {
        primed = true;
        spoken = next;
        return;
      }
      cancel();
      if (next === spoken) return;
      timer = timers.set(() => {
        timer = null;
        spoken = next;
        if (next) say(SPOKEN_WORDS[next]);
      }, SETTLE_MS);
    },
    /** Another thread: its first status is not news. */
    reset() {
      cancel();
      primed = false;
      spoken = null;
      say("");
    },
    dispose: cancel,
  };
}

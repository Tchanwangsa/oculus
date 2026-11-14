import { isWaiting, type Download, type DownloadState } from "./types";

/**
 * Which download the Save dialog is asking about. A new download asks only
 * when it started in a browser tab in front; one in a background tab just
 * waits in the list. A dismissed dialog never comes back on its own, but a
 * delivery that failed (the row is `ready` again with an `error`) asks
 * again, as a new download would.
 *
 * Rust credits a blank popup's download to the tab that opened it and closes
 * the popup, so the popup can be in front when the download arrives. An
 * arrival is therefore decided once the tabs settle: when a tab in front at
 * arrival has closed and the download's tab is in front, it asks. One whose
 * tab was not in front waits `ARRIVAL_GRACE_MS` for that to happen.
 */

/** How long a download from a tab not in front waits for the popup in front
 *  to close and hand the front back to its opener. */
export const ARRIVAL_GRACE_MS = 3000;

interface Arrival {
  since: number;
  /** The browser tabs in front when it arrived. */
  front: number[];
}

export interface Prompt {
  /** The download the dialog shows, or null. */
  open: number | null;
  /** Arrivals not yet decided, by download id. */
  arrivals: Record<number, Arrival>;
  /** Each row's last seen state, to tell a new row or a failed delivery. */
  states: Record<number, DownloadState>;
  /** The browser tabs in front at the last step. */
  front: number[];
}

/** The browser tabs on show in the tab in front, and every live tab. */
export interface View {
  front: readonly number[];
  live: ReadonlySet<number>;
}

export const EMPTY_PROMPT: Prompt = { open: null, arrivals: {}, states: {}, front: [] };

function sameTabs(a: readonly number[], b: readonly number[]): boolean {
  if (a.length !== b.length) return false;
  const sorted = [...b].sort((x, y) => x - y);
  return [...a].sort((x, y) => x - y).every((t, i) => t === sorted[i]);
}

type Verdict = "ask" | "background" | "wait";

function decide(arrival: Arrival, tab: number, view: View, now: number): Verdict {
  const inFront = view.front.includes(tab);
  if (arrival.front.includes(tab)) return inFront ? "ask" : "background";
  if (arrival.front.some((t) => !view.live.has(t))) {
    if (inFront) return "ask";
    // The strip still shows the closed popup until it catches up.
    return view.front.some((t) => !view.live.has(t)) ? "wait" : "background";
  }
  if (arrival.front.length === 0) return "background";
  return now - arrival.since < ARRIVAL_GRACE_MS ? "wait" : "background";
}

/**
 * The prompt after a new list, a change of tabs in front, or a tick. `initial`
 * is the mount's fetch: its rows are learnt without asking, since they
 * started before this view of the app existed.
 */
export function step(
  prev: Prompt,
  downloads: readonly Download[],
  view: View,
  now: number,
  initial = false,
): Prompt {
  // Switching tabs dismisses the dialog; the download waits in the list.
  let open = sameTabs(prev.front, view.front) ? prev.open : null;
  const arrivals = { ...prev.arrivals };
  const states: Record<number, DownloadState> = {};

  for (const d of downloads) {
    const before = prev.states[d.id];
    states[d.id] = d.state;
    const arrived = before === undefined ? !initial && isWaiting(d) : false;
    const bounced = before !== undefined && before !== "ready" && d.state === "ready" && d.error != null;
    if (arrived || bounced) arrivals[d.id] = { since: now, front: [...view.front] };
  }

  const byId = new Map(downloads.map((d) => [d.id, d]));
  const shown = open == null ? undefined : byId.get(open);
  if (!shown || !isWaiting(shown)) open = null;

  for (const key of Object.keys(arrivals)) {
    const id = Number(key);
    const d = byId.get(id);
    if (!d || !isWaiting(d)) {
      delete arrivals[id];
      continue;
    }
    const verdict = decide(arrivals[id], d.tab, view, now);
    if (verdict === "wait") continue;
    delete arrivals[id];
    if (verdict === "ask" && open == null) open = id;
  }

  return { open, arrivals, states, front: [...view.front] };
}

/** The student picked a waiting row in the list. */
export function ask(prev: Prompt, downloads: readonly Download[], id: number): Prompt {
  const d = downloads.find((x) => x.id === id);
  if (!d || !isWaiting(d)) return prev;
  const arrivals = { ...prev.arrivals };
  delete arrivals[id];
  return { ...prev, open: id, arrivals };
}

/** Escape, a click away, or a choice made: the download stays in the list. */
export function dismiss(prev: Prompt): Prompt {
  return prev.open == null ? prev : { ...prev, open: null };
}

/** When the next undecided arrival gives up waiting, for a tick; null if
 *  none waits. */
export function nextDeadline(prompt: Prompt): number | null {
  const times = Object.values(prompt.arrivals).map((a) => a.since + ARRIVAL_GRACE_MS);
  return times.length ? Math.min(...times) : null;
}

/**
 * Which lecture player holds the app's `<video>` elements
 * (`lib/lectures/playback/`). A tab's main page and its side panel's front
 * item can both be lecture pages, so two players can be on screen at once;
 * this decides which one adopts the elements. No DOM here, so it is testable.
 *
 * Ownership moves only when:
 * - a player claims it on a user action (`claimPlayback`: play, seek, Space)
 *   or the side panel's expand hands it over, naming the lecture the new
 *   owner must be showing;
 * - the owner's tab goes to the back, and a player in the tab now in front
 *   adopts the elements;
 * - the owner unmounts paused (`releasePlayback`) or playback stops, leaving
 *   the elements to whichever player is on screen.
 * A player in the owner's own tab never takes them on mount.
 */

export interface PlaybackOwner {
  /** The pane whose player owns playback, or null for none. */
  pane: number | null;
  /** That pane's strip tab. */
  tab: number | null;
  /** Set by a claim: the lecture the owner must show before it adopts. */
  lectureId: string | null;
  /** The leader element is playing. */
  playing: boolean;
}

const NOBODY: PlaybackOwner = {
  pane: null,
  tab: null,
  lectureId: null,
  playing: false,
};

/**
 * Whether the on-screen player in `pane` (strip tab `tab`, showing
 * `lectureId`) may adopt the elements. A neighbour in the owner's tab waits:
 * the owner is on screen, coming back with its tab, or about to adopt after a
 * claim. A player in any other tab is on screen, so the owner's tab is not,
 * and it takes over (the claim token stops the owner parking them after).
 */
export function mayAdopt(
  s: PlaybackOwner,
  pane: number,
  tab: number,
  lectureId: string,
): boolean {
  if (s.pane === pane) return s.lectureId == null || s.lectureId === lectureId;
  return s.pane == null || s.tab !== tab;
}

let state: PlaybackOwner = NOBODY;
const listeners = new Set<() => void>();

function update(patch: Partial<PlaybackOwner>) {
  const next = { ...state, ...patch };
  if ((Object.keys(patch) as (keyof PlaybackOwner)[]).every((k) => next[k] === state[k])) return;
  state = next;
  for (const fn of listeners) fn();
}

/** The current owner; a new object on every change, for `useSyncExternalStore`. */
export function playbackOwner(): PlaybackOwner {
  return state;
}

export function subscribePlaybackOwner(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** Playback now belongs to `pane`, but its player adopts the elements only
 *  while showing `lectureId`; the previous owner's player lets go. */
export function claimPlayback(pane: number, tab: number, lectureId: string): void {
  update({ pane, tab, lectureId });
}

/** A player moved the elements into its frames. */
export function markAdopted(pane: number, tab: number): void {
  update({ pane, tab, lectureId: null });
}

export function markPlaying(playing: boolean): void {
  update({ playing });
}

/** The player in `pane` unmounted. A paused owner gives playback up, so the
 *  other pane's player can take it; a playing one keeps it, playing on parked
 *  (an expand's new pane has already been claimed for). */
export function releasePlayback(pane: number): void {
  if (state.pane !== pane || state.playing) return;
  update({ pane: null, tab: null, lectureId: null });
}

/** Playback stopped for good: nobody owns it. */
export function clearPlaybackOwner(): void {
  update(NOBODY);
}

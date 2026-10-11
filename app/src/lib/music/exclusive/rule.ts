/**
 * Music and the app's own sound (a lecture, a library video) never play
 * together: the one started last wins and the other pauses. Nothing resumes
 * by itself. Pure, so it is testable; `./index.ts` wires it. See docs/music.md.
 */

export interface Sounds {
  music: boolean;
  /** A lecture or a library video. */
  local: boolean;
}

/** Which side to pause after `prev` became `next`, if any. Both starting at
 *  once keeps the app's own: that one was a click here. */
export function toSilence(prev: Sounds, next: Sounds): keyof Sounds | null {
  const musicStarted = !prev.music && next.music;
  const localStarted = !prev.local && next.local;
  if (localStarted && next.music) return "music";
  if (musicStarted && next.local) return "local";
  return null;
}

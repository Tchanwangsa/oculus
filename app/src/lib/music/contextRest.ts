import type { MusicNextTrack } from "./index";

/** A context whose tracks Up next can continue with. */
export function continuable(contextUri: string | null): contextUri is string {
  return (
    contextUri !== null &&
    (contextUri.startsWith("spotify:playlist:") || contextUri.startsWith("spotify:album:"))
  );
}

/**
 * Where the context carries on after Spotify's own next tracks: the position
 * in `context` just past the last non-queued `upNext` track (the anchor).
 * With the anchor listed more than once, its first place after the current
 * track's; else its first place. Null with no anchor, or one the context
 * does not list.
 */
export function contextRestStart(
  context: string[],
  upNext: MusicNextTrack[],
  currentUri: string | null,
): number | null {
  const anchor = [...upNext].reverse().find((t) => !t.queued)?.uri;
  if (anchor === undefined) return null;
  const current = currentUri === null ? -1 : context.indexOf(currentUri);
  let at = current < 0 ? -1 : context.indexOf(anchor, current + 1);
  if (at < 0) at = context.indexOf(anchor);
  return at < 0 ? null : at + 1;
}

import { useEffect, useMemo, useState } from "react";
import { music, type MusicNextTrack } from "@/lib/music";
import { continuable, contextRestStart } from "@/lib/music/contextRest";

const NONE: string[] = [];

/** The last context's tracks, so reopening Up next asks nothing. */
let kept: { uri: string; tracks: string[] } | null = null;

/**
 * The rest of the playing playlist or album after Spotify's own next tracks
 * (`contextRestStart`), as Spotify's desktop queue shows it: the context's
 * track URIs, fetched once per context and kept while it stays the same.
 * Empty while shuffling, for any other context, until the tracks arrive, or
 * when they cannot be fetched. Stable while its inputs stay the same.
 */
export function useContextRest(
  contextUri: string | null,
  shuffle: boolean,
  upNext: MusicNextTrack[],
  currentUri: string | null,
): string[] {
  const on = continuable(contextUri) && !shuffle;
  const [context, setContext] = useState(() =>
    on && kept?.uri === contextUri ? kept : null,
  );

  useEffect(() => {
    if (!on || contextUri === null) return;
    if (kept?.uri === contextUri) {
      setContext(kept);
      return;
    }
    let live = true;
    setContext(null);
    const tracks = contextUri.startsWith("spotify:album:")
      ? music.albumTracks
      : music.playlistTracks;
    tracks(contextUri).then(
      (list) => {
        kept = { uri: contextUri, tracks: list };
        if (live) setContext(kept);
      },
      () => {},
    );
    return () => {
      live = false;
    };
  }, [on, contextUri]);

  const tracks = on && context?.uri === contextUri ? context.tracks : null;
  const start = tracks ? contextRestStart(tracks, upNext, currentUri) : null;
  return useMemo(
    () => (tracks && start !== null ? tracks.slice(start) : NONE),
    [tracks, start],
  );
}

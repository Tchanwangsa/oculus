import { useEffect, useRef, useState } from "react";
import { music, TRACK_PAGE, type MusicTrackRow } from "@/lib/music";

/**
 * Rows for `uris` (null for a track Spotify describes nothing for; absent
 * while unfetched), fetched a `TRACK_PAGE` at a time as `pages` comes to need
 * them (the pages a virtual list shows). Rust keeps every row it fetched, so
 * asking again costs no request; a page that failed (a rate limit) is asked
 * for again the next time it is needed. Pass a stable `uris` array: a new
 * one starts the pages over.
 */
export function useTrackRows(
  uris: string[],
  pages: number[],
): Record<string, MusicTrackRow | null> {
  const [rows, setRows] = useState<Record<string, MusicTrackRow | null>>({});
  const asked = useRef<{ uris: string[]; pages: Set<number> }>({ uris, pages: new Set() });
  const key = pages.join(",");

  useEffect(() => {
    if (asked.current.uris !== uris) asked.current = { uris, pages: new Set() };
    const mine = asked.current;
    for (const page of pages) {
      if (mine.pages.has(page)) continue;
      const slice = uris.slice(page * TRACK_PAGE, (page + 1) * TRACK_PAGE);
      if (slice.length === 0) continue;
      mine.pages.add(page);
      music.tracks(slice).then(
        (found) =>
          setRows((prev) => {
            const next = { ...prev };
            for (const uri of slice) next[uri] = found[uri] ?? null;
            return next;
          }),
        () => mine.pages.delete(page),
      );
    }
    // `key` stands for `pages`, which is a new array on every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [uris, key]);

  return rows;
}

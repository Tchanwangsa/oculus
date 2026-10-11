import { useMemo, useRef } from "react";
import { useTrackRows } from "@/hooks/music/useTrackRows";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import type { MusicNextTrack, MusicSnapshot } from "@/lib/music";
import { TrackRow } from "./TrackRow";

const FIRST_PAGE = [0];

/**
 * What plays next, from the snapshot's `up_next`: queued tracks under
 * Queued, then the rest of the context under Next. Rows are not controls:
 * librespot has no command to skip to one. Nothing shows while the list is
 * empty.
 */
export function UpNext({ snapshot }: { snapshot: MusicSnapshot }) {
  const next = snapshot.up_next;
  const key = next.map((t) => t.uri).join("|");
  // A stable list while the URIs stay the same: each snapshot is a new array.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const uris = useMemo(() => next.map((t) => t.uri), [key]);
  const rows = useTrackRows(uris, FIRST_PAGE);
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef, "y", next.length > 0);
  if (next.length === 0) return null;

  const queued = next.filter((t) => t.queued);
  const rest = next.filter((t) => !t.queued);
  const group = (label: string, tracks: MusicNextTrack[]) =>
    tracks.length === 0 ? null : (
      <section>
        <h3 className="px-2 pb-1 text-xs font-medium text-muted-foreground">{label}</h3>
        {tracks.map((t, i) => (
          <TrackRow
            key={`${t.uri}-${i}`}
            uri={t.uri}
            row={rows[t.uri]}
            current={false}
            disabled={false}
            queue={false}
          />
        ))}
      </section>
    );

  return (
    <div className="flex flex-col gap-2">
      <h2 className="flex h-6 items-center px-2 text-xs font-medium text-muted-foreground">
        Up next
      </h2>
      <div ref={scrollRef} className="max-h-60 overflow-y-auto">
        <div className="flex flex-col gap-3">
          {group("Queued", queued)}
          {group("Next", rest)}
        </div>
      </div>
    </div>
  );
}

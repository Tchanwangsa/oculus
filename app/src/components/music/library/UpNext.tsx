import { useMemo, useRef } from "react";
import { useTrackRows } from "@/hooks/music/useTrackRows";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import type { MusicNextTrack, MusicSnapshot } from "@/lib/music";
import { TrackRow } from "./TrackRow";

const FIRST_PAGE = [0];

/**
 * What plays next, from the snapshot's `up_next`: queued tracks, then the
 * rest of the context. The two are labelled Queued and Then only while there
 * are queued tracks; otherwise it is one list. Rows are not controls:
 * librespot has no command to skip to one. Shown in the player's queue view,
 * where it fills the column like the library it replaces.
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

  const queued = next.filter((t) => t.queued);
  const rest = next.filter((t) => !t.queued);
  const group = (label: string | null, tracks: MusicNextTrack[]) =>
    tracks.length === 0 ? null : (
      <section>
        {label && (
          <h3 className="px-2 pb-1 text-xs font-medium text-muted-foreground">{label}</h3>
        )}
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
    <div className="flex flex-col gap-2 @3xl:min-h-0 @3xl:flex-1">
      <h2 className="flex h-6 items-center px-2 text-xs font-medium text-muted-foreground">
        Up next
      </h2>
      {next.length === 0 ? (
        <p className="px-2 text-xs text-muted-foreground">Nothing up next.</p>
      ) : (
        <div ref={scrollRef} className="max-h-96 overflow-y-auto @3xl:max-h-none @3xl:min-h-0 @3xl:flex-1">
          <div className="flex flex-col gap-3">
            {group("Queued", queued)}
            {group(queued.length > 0 ? "Then" : null, rest)}
          </div>
        </div>
      )}
    </div>
  );
}

import { useMemo } from "react";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import { useTrackRows } from "@/hooks/music/useTrackRows";
import { music, trackPages, type MusicPlayed } from "@/lib/music";
import { useMusicStore } from "@/stores/music/musicStore";
import { TrackRow } from "./TrackRow";

const NO_TRACKS: string[] = [];

/**
 * The Recently played tab under the player: the tracks the account played
 * lately, newest first, as track rows (`useTrackRows`; at most 50, so no
 * virtual list). A click plays the track from its place in its album, as a
 * search hit does; the track playing now is in brand colour.
 */
export function RecentlyPlayed({
  played,
  error,
  currentUri,
  disabled,
}: {
  played: MusicPlayed[] | null;
  error: string | null;
  currentUri: string | null;
  disabled: boolean;
}) {
  const run = useMusicStore((s) => s.run);
  const uris = useMemo(() => played?.map((p) => p.uri) ?? NO_TRACKS, [played]);
  const pages = useMemo(() => trackPages(0, uris.length - 1), [uris]);
  const rows = useTrackRows(uris, pages);
  return (
    <>
      {error && <p className="px-2 pb-1 text-xs text-destructive">{error}</p>}
      {played === null ? (
        !error && <SkeletonRows count={4} rowClassName="h-11" className="px-2" />
      ) : played.length === 0 ? (
        !error && <p className="px-2 text-xs text-muted-foreground">Nothing played recently.</p>
      ) : (
        uris.map((uri) => (
          <TrackRow
            key={uri}
            uri={uri}
            row={rows[uri]}
            current={uri === currentUri}
            disabled={disabled}
            onPlay={() => void run(() => music.playTrack(uri, rows[uri]?.album_uri ?? null))}
          />
        ))
      )}
    </>
  );
}

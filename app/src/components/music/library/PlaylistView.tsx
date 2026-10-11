import { useEffect, useMemo, useRef, useState } from "react";
import { Play } from "@phosphor-icons/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Button } from "@/components/ui/button";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import { useTrackRows } from "@/hooks/music/useTrackRows";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { music, playlistSubtitle, trackPages, type MusicPlaylist } from "@/lib/music";
import { useMusicStore } from "@/stores/music/musicStore";
import { PlaylistCover } from "./PlaylistCover";
import { TrackRow } from "./TrackRow";

/** A row's height: `LibraryRow`'s 36px cover and its padding. */
const ROW = 48;
const NO_TRACKS: string[] = [];

/**
 * One playlist opened in the library: its header with a Play button, and its
 * tracks as a virtual list, since a playlist runs to thousands. Rust sends
 * every track's URI at once; rows are fetched a hundred at a time as they
 * scroll into view (`useTrackRows`). A row plays from its place in the
 * playlist.
 */
export function PlaylistView({
  playlist,
  currentUri,
  disabled,
}: {
  playlist: MusicPlaylist;
  currentUri: string | null;
  disabled: boolean;
}) {
  const run = useMusicStore((s) => s.run);
  const [uris, setUris] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let live = true;
    setUris(null);
    setError(null);
    music.playlistTracks(playlist.uri).then(
      (list) => live && setUris(list),
      (e: unknown) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, [playlist.uri]);

  const list = uris ?? NO_TRACKS;
  useScrollFade(scrollRef, "y", list.length);
  const virtualizer = useVirtualizer({
    count: list.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW,
    overscan: 10,
  });
  const items = virtualizer.getVirtualItems();
  const first = items[0]?.index ?? 0;
  const last = items[items.length - 1]?.index ?? -1;
  const pages = useMemo(() => trackPages(first, last), [first, last]);
  const rows = useTrackRows(list, pages);

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-3 px-2">
        <PlaylistCover images={playlist.images} className="size-14 rounded-lg" iconSize={20} />
        <div className="min-w-0 flex-1">
          <p className="truncate font-display text-sm font-semibold text-foreground">
            {playlist.name}
          </p>
          <p className="truncate text-xs text-muted-foreground tabular-nums">
            {playlistSubtitle(playlist)}
          </p>
        </div>
        <Button
          size="xs"
          disabled={disabled}
          onClick={() => void run(() => music.playContext(playlist.uri))}
        >
          <Play weight="fill" />
          Play
        </Button>
      </div>
      {error ? (
        <p className="px-2 text-xs text-destructive">{error}</p>
      ) : uris === null ? (
        <SkeletonRows count={4} rowClassName="h-11" className="px-2" />
      ) : uris.length === 0 ? (
        <p className="px-2 text-xs text-muted-foreground">No tracks in this playlist.</p>
      ) : (
        <div ref={scrollRef} className="max-h-96 overflow-y-auto">
          <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {items.map((item) => {
              const uri = list[item.index];
              return (
                <div
                  key={item.key}
                  className="absolute inset-x-0"
                  style={{ top: item.start, height: item.size }}
                >
                  <TrackRow
                    uri={uri}
                    row={rows[uri]}
                    current={uri === currentUri}
                    disabled={disabled}
                    onPlay={() => void run(() => music.playTrack(uri, playlist.uri))}
                  />
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

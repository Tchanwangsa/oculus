import { Skeleton } from "@/components/ui/skeleton";
import { trackSubtitle, type MusicTrackRow } from "@/lib/music";
import { LibraryRow } from "./LibraryRow";

/** A track by its fetched row: a skeleton until it arrives, a muted line for
 *  one Spotify describes nothing for. */
export function TrackRow({
  uri,
  row,
  current,
  disabled,
  onPlay,
  onRemove,
}: {
  uri: string;
  row: MusicTrackRow | null | undefined;
  current: boolean;
  disabled: boolean;
  onPlay?: () => void;
  /** Offer "Remove from queue" (up next), which calls this. */
  onRemove?: () => void;
}) {
  if (row === undefined) {
    return (
      <div className="flex items-center gap-3 px-2 py-1.5">
        <Skeleton className="size-9 rounded-md" />
        <Skeleton className="h-3.5 flex-1" />
      </div>
    );
  }
  if (row === null) {
    return <LibraryRow name="Unavailable track" subtitle="" images={[]} onRemove={onRemove} />;
  }
  return (
    <LibraryRow
      name={row.name}
      subtitle={trackSubtitle(row)}
      images={row.image_url ? [row.image_url] : []}
      durationMs={row.duration_ms}
      queueUri={uri}
      current={current}
      disabled={disabled}
      onPlay={onPlay}
      onRemove={onRemove}
    />
  );
}

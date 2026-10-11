import { Button } from "@/components/ui/button";
import type { MusicSnapshot } from "@/lib/music";
import { MusicCover } from "./MusicCover";
import { MusicSeek } from "./MusicSeek";
import { MusicTransport } from "./MusicTransport";
import { PlayingElsewhere } from "./PlayingElsewhere";
import { VolumeRow } from "./VolumeRow";

/** What nothing playing says in the popover, which has no list to pick from. */
export const NOTHING_PLAYING_HINT = "Choose Oculus as the device in Spotify.";

/** The rail popover's player: the track, seek, transport, volume and devices,
 *  and the way to the full player. */
export function NowPlaying({
  snapshot,
  error,
  onOpenPlayer,
}: {
  snapshot: MusicSnapshot;
  error: string | null;
  onOpenPlayer: () => void;
}) {
  const track = snapshot.track;
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-3">
        <MusicCover url={track?.cover_url} className="size-12 rounded-md" />
        <div className="min-w-0">
          <p className="truncate text-[13px] font-medium text-foreground">
            {track?.name ?? (snapshot.connecting ? "Connecting…" : "Nothing playing")}
          </p>
          <p className="truncate text-xs text-muted-foreground">
            {track ? track.artists.join(", ") : NOTHING_PLAYING_HINT}
          </p>
        </div>
      </div>
      {track && <MusicSeek snapshot={snapshot} />}
      <MusicTransport snapshot={snapshot} />
      <VolumeRow snapshot={snapshot} />
      <PlayingElsewhere snapshot={snapshot} />
      {error && <p className="text-xs text-destructive">{error}</p>}
      <Button variant="ghost" size="xs" className="self-center" onClick={onOpenPlayer}>
        Open player
      </Button>
    </div>
  );
}

import { Button } from "@/components/ui/button";
import { shownPlayback, type MusicSnapshot } from "@/lib/music";
import { MusicCover } from "./MusicCover";
import { connectingTitle, MusicNotice } from "./MusicNotice";
import { MusicSeek } from "./MusicSeek";
import { MusicTransport } from "./MusicTransport";
import { PlayingElsewhere } from "./PlayingElsewhere";
import { VolumeRow } from "./VolumeRow";

/** What nothing playing says in the popover, which has no list to pick from. */
export const NOTHING_PLAYING_HINT = "Choose Oculus as the device in Spotify.";

/** The title without a track: another device's item not named yet (an
 *  episode, or a row still on its way), or nothing at all. */
export function nothingNamed(device: string | null): string {
  return device ? `Playing on ${device}` : "Nothing playing";
}

/** The rail popover's player: the track (`shownPlayback`: the device playing
 *  elsewhere, else Oculus), seek, transport, volume and devices, and the way
 *  to the full player. */
export function NowPlaying({
  snapshot,
  error,
  onOpenPlayer,
}: {
  snapshot: MusicSnapshot;
  error: string | null;
  onOpenPlayer: () => void;
}) {
  const { track, device } = shownPlayback(snapshot);
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-3">
        <MusicCover url={track?.cover_url} className="size-12 rounded-md" />
        <div className="min-w-0">
          <p className="truncate text-[13px] font-medium text-foreground">
            {track?.name ?? connectingTitle(snapshot) ?? nothingNamed(device)}
          </p>
          <p className="truncate text-xs text-muted-foreground">
            {track ? track.artists.join(", ") : device ? "" : NOTHING_PLAYING_HINT}
          </p>
        </div>
      </div>
      {track && <MusicSeek snapshot={snapshot} />}
      <MusicTransport snapshot={snapshot} />
      <VolumeRow snapshot={snapshot} />
      <PlayingElsewhere snapshot={snapshot} />
      <MusicNotice snapshot={snapshot} error={error} />
      <Button variant="ghost" size="xs" className="self-center" onClick={onOpenPlayer}>
        Open player
      </Button>
    </div>
  );
}

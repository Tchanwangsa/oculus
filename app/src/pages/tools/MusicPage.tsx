import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { MusicNotes } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/layout/PageParts";
import { useTabActive } from "@/components/tabs/TabContext";
import { MusicCover } from "@/components/music/MusicCover";
import { connectingTitle, MusicNotice } from "@/components/music/MusicNotice";
import { RepeatToggle, ShuffleToggle } from "@/components/music/MusicModes";
import { MusicSeek } from "@/components/music/MusicSeek";
import { MusicTransport } from "@/components/music/MusicTransport";
import { nothingNamed } from "@/components/music/NowPlaying";
import { PlayingElsewhere } from "@/components/music/PlayingElsewhere";
import { QueueToggle } from "@/components/music/QueueToggle";
import { VolumeRow } from "@/components/music/VolumeRow";
import { useMusicSpaceKey } from "@/hooks/music/useMusicSpaceKey";
import { MusicLibrary } from "@/components/music/library/MusicLibrary";
import { UpNext } from "@/components/music/library/UpNext";
import { music, shownPlayback, type MusicSnapshot } from "@/lib/music";
import { cn } from "@/lib/utils";
import { musicError, useMusicStore } from "@/stores/music/musicStore";
import { useMusicPrefsStore } from "@/stores/music/musicPrefsStore";

/** Why there is no player, for the empty view. */
function notConnectedBody(enabled: boolean, s: MusicSnapshot | null): string {
  if (!enabled) return "Spotify is off. Turn it on in Settings → Music.";
  if (s?.needs_login) return "Log in to Spotify in Settings → Music.";
  return s?.error ?? "Connect to Spotify in Settings → Music.";
}

/** What nothing playing says here, above the list to pick from. */
const NOTHING_PLAYING = "Play a song here or from any device.";

/**
 * The full player, opened as the page from the rail's popover (`/music`),
 * with the account's playlists and search, or what plays next behind the
 * queue button. Narrow, it is one column; from `@3xl` of its own width the
 * player sits left and the list fills the height on the right, the list
 * being the only scroller. Spotify's state is Rust's (`useMusicState`); this
 * only draws it and sends commands. Without a session or a track it points
 * to Settings → Music. Space plays and pauses (`useMusicSpaceKey`). Coming
 * into view sends `music.wake`, so a connection that died meanwhile is made
 * again at once.
 */
export default function MusicPage() {
  const active = useTabActive();
  const enabled = useMusicPrefsStore((s) => s.enabled);
  const snapshot = useMusicStore((s) => s.snapshot);
  const error = useMusicStore(musicError);
  const [queueOpen, setQueueOpen] = useState(false);
  useMusicSpaceKey(snapshot);
  useEffect(() => {
    if (active) music.wake().catch(() => {});
  }, [active]);

  if (!enabled || !snapshot || !(snapshot.connected || snapshot.connecting || snapshot.track)) {
    return (
      <div className="page-scroll">
        <div className="mx-auto max-w-md px-6 py-10">
          <EmptyState
            icon={<MusicNotes size={22} className="text-muted-foreground" />}
            title="Not connected"
            body={notConnectedBody(enabled, snapshot)}
          >
            <Button variant="outline" size="xs" asChild>
              <Link to="/settings/music">Open Settings → Music</Link>
            </Button>
          </EmptyState>
        </div>
      </div>
    );
  }

  const { track, device } = shownPlayback(snapshot);
  return (
    <div className="@container h-full">
      <div className="page-scroll @3xl:overflow-y-hidden">
        <div className="mx-auto flex max-w-sm flex-col gap-5 px-6 py-8 @3xl:h-full @3xl:max-w-5xl @3xl:flex-row @3xl:gap-10 @3xl:py-6">
          <div className="flex flex-col @3xl:-mx-2 @3xl:w-84 @3xl:shrink-0 @3xl:overflow-y-auto @3xl:px-2">
            <div className="flex flex-col gap-5 @3xl:my-auto">
              <MusicCover
                url={track?.cover_url}
                className="aspect-square w-full rounded-xl"
                iconSize={48}
              />
              <div className="min-w-0 text-center">
                <h1 className="truncate text-lg font-semibold text-foreground">
                  {track?.name ?? connectingTitle(snapshot) ?? nothingNamed(device)}
                </h1>
                <p className="truncate text-[13px] text-foreground/80">
                  {track ? track.artists.join(", ") : device ? "" : NOTHING_PLAYING}
                </p>
                {track?.album && (
                  <p className="truncate text-xs text-muted-foreground">{track.album}</p>
                )}
              </div>
              <MusicSeek snapshot={snapshot} active={active} />
              <div className="flex items-center justify-between">
                <ShuffleToggle snapshot={snapshot} />
                <MusicTransport snapshot={snapshot} large />
                <RepeatToggle snapshot={snapshot} />
              </div>
              <VolumeRow snapshot={snapshot}>
                <QueueToggle open={queueOpen} onToggle={() => setQueueOpen((o) => !o)} />
              </VolumeRow>
              <PlayingElsewhere snapshot={snapshot} />
              <MusicNotice snapshot={snapshot} error={error} />
            </div>
          </div>
          <div className="flex min-w-0 flex-col @3xl:min-h-0 @3xl:flex-1">
            {queueOpen && <UpNext snapshot={snapshot} />}
            {/* Kept mounted under the queue, so an open playlist, its scroll
                and a search are still there when the queue closes. */}
            <div className={cn("flex flex-col @3xl:min-h-0 @3xl:flex-1", queueOpen && "hidden")}>
              <MusicLibrary snapshot={snapshot} />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

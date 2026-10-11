import { Link } from "react-router-dom";
import { MusicNotes } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/layout/PageParts";
import { useTabActive } from "@/components/tabs/TabContext";
import { MusicCover } from "@/components/music/MusicCover";
import { RepeatToggle, ShuffleToggle } from "@/components/music/MusicModes";
import { MusicSeek } from "@/components/music/MusicSeek";
import { MusicTransport } from "@/components/music/MusicTransport";
import { PlayingElsewhere } from "@/components/music/PlayingElsewhere";
import { VolumeRow } from "@/components/music/VolumeRow";
import { MusicLibrary } from "@/components/music/library/MusicLibrary";
import type { MusicSnapshot } from "@/lib/music";
import { musicError, useMusicStore } from "@/stores/music/musicStore";
import { useMusicPrefsStore } from "@/stores/music/musicPrefsStore";

/** Why there is no player, for the empty view. */
function notConnectedBody(enabled: boolean, s: MusicSnapshot | null): string {
  if (!enabled) return "Spotify is off. Turn it on in Settings → Music.";
  if (s?.needs_login) return "Log in to Spotify in Settings → Music.";
  return s?.error ?? "Connect to Spotify in Settings → Music.";
}

/** What nothing playing says here, above the list to pick from. */
const NOTHING_PLAYING = "Pick a playlist below, or choose Oculus in Spotify.";

/**
 * The full player, opened beside the page from the rail's popover (`/music`),
 * with the account's playlists and search under it. Spotify's state is
 * Rust's (`useMusicState`); this only draws it and sends commands. Without a
 * session or a track it points to Settings → Music.
 */
export default function MusicPage() {
  const active = useTabActive();
  const enabled = useMusicPrefsStore((s) => s.enabled);
  const snapshot = useMusicStore((s) => s.snapshot);
  const error = useMusicStore(musicError);

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

  const track = snapshot.track;
  return (
    <div className="page-scroll">
      <div className="mx-auto flex max-w-sm flex-col gap-5 px-6 py-8">
        <MusicCover
          url={track?.cover_url}
          className="aspect-square w-full rounded-xl"
          iconSize={48}
        />
        <div className="min-w-0 text-center">
          <h1 className="truncate text-lg font-semibold text-foreground">
            {track?.name ?? (snapshot.connecting ? "Connecting…" : "Nothing playing")}
          </h1>
          <p className="truncate text-[13px] text-foreground/80">
            {track ? track.artists.join(", ") : NOTHING_PLAYING}
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
        <VolumeRow snapshot={snapshot} />
        <PlayingElsewhere snapshot={snapshot} />
        {error && <p className="text-xs text-destructive">{error}</p>}
        <MusicLibrary snapshot={snapshot} />
      </div>
    </div>
  );
}

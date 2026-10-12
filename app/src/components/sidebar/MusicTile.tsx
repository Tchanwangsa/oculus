import { useState } from "react";
import { CircleNotch, MusicNotes } from "@phosphor-icons/react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { NowPlaying } from "@/components/music/NowPlaying";
import { music, shownPlayback } from "@/lib/music";
import { openBeside } from "@/lib/shell/tabRouters";
import { cn } from "@/lib/utils";
import { musicError, useMusicStore } from "@/stores/music/musicStore";
import { useMusicPrefsStore } from "@/stores/music/musicPrefsStore";
import { useActivePath, useTabStore } from "@/stores/shell/tabStore";
import { railButton, railIdle } from "./RailItem";

/**
 * Spotify in the rail's group after the sections: the album cover (or a
 * note) of what the player shows — Oculus, or the device playing elsewhere —
 * with the rail's corner marks: a spinner while connecting, a brand dot
 * while playing. A popover player opens to its right, and opening it sends
 * `music.wake` so a connection that died meanwhile is made again at once.
 * Only while `useMusicTileShown`; otherwise nothing at all.
 */
export function MusicTile() {
  const shown = useMusicTileShown();
  const snapshot = useMusicStore((s) => s.snapshot);
  const error = useMusicStore(musicError);
  const [open, setOpen] = useState(false);
  const path = useActivePath().split("?")[0];
  const isActive = open || path === "/music" || path.startsWith("/music/");

  if (!shown || !snapshot) return null;
  const { track, playing } = shownPlayback(snapshot);
  const named = track ? `${track.name} · ${track.artists.join(", ")}` : "Music";
  const label = snapshot.reconnecting ? `${named} · Reconnecting…` : named;
  const indicator = snapshot.connecting ? (
    <CircleNotch size={10} weight="bold" className="animate-spin text-brand" />
  ) : playing ? (
    // Ringed in the rail's ground, so it reads over a cover.
    <span className="size-1.5 rounded-full bg-brand ring-2 ring-background" />
  ) : null;

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (next) music.wake().catch(() => {});
      }}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger asChild>
            <button
              type="button"
              aria-label={label}
              className={cn(railButton, isActive ? "bg-sidebar-item-active text-foreground" : railIdle)}
            >
              {track?.cover_url ? (
                <img
                  src={track.cover_url}
                  alt=""
                  draggable={false}
                  className="size-6 rounded-[5px] object-cover"
                />
              ) : (
                <MusicNotes size={18} weight={isActive ? "fill" : "regular"} />
              )}
              {indicator && (
                <span aria-hidden className="absolute top-1 right-1 flex">
                  {indicator}
                </span>
              )}
            </button>
          </PopoverTrigger>
        </TooltipTrigger>
        {!open && <TooltipContent side="right">{label}</TooltipContent>}
      </Tooltip>
      <PopoverContent side="right" align="start" sideOffset={8} className="w-72">
        <NowPlaying
          snapshot={snapshot}
          error={error}
          onOpenPlayer={(newTab) => {
            setOpen(false);
            if (newTab) useTabStore.getState().addTab("/music");
            else openBeside("/music");
          }}
        />
      </PopoverContent>
    </Popover>
  );
}

/** Settings → Music is on and a session is up or a track is loaded. */
export function useMusicTileShown() {
  const enabled = useMusicPrefsStore((s) => s.enabled);
  const snapshot = useMusicStore((s) => s.snapshot);
  return enabled && !!snapshot && (snapshot.connected || !!snapshot.track);
}

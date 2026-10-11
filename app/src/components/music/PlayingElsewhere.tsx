import { Devices } from "@phosphor-icons/react";
import type { MusicSnapshot } from "@/lib/music";

/** "Playing on <device>" while another Connect device holds playback; any
 *  transport command pulls it back here. */
export function PlayingElsewhere({ snapshot }: { snapshot: MusicSnapshot }) {
  if (snapshot.active || !snapshot.playing_on) return null;
  return (
    <p className="flex min-w-0 items-center justify-center gap-1.5 text-xs text-brand">
      <Devices size={13} className="shrink-0" />
      <span className="truncate">Playing on {snapshot.playing_on}</span>
    </p>
  );
}

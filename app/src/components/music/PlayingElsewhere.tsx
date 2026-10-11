import { Devices } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { music, type MusicSnapshot } from "@/lib/music";
import { useMusicStore } from "@/stores/music/musicStore";

/** "Playing on <device>" while another Connect device holds playback, which
 *  the transport then controls where it is; "Play here" moves it to Oculus. */
export function PlayingElsewhere({ snapshot }: { snapshot: MusicSnapshot }) {
  const run = useMusicStore((s) => s.run);
  if (snapshot.active || !snapshot.playing_on) return null;
  return (
    <div className="flex min-w-0 items-center justify-center gap-1.5 text-xs text-brand">
      <Devices size={13} className="shrink-0" />
      <span className="truncate">Playing on {snapshot.playing_on}</span>
      <Button
        variant="ghost"
        size="xs"
        className="shrink-0"
        disabled={!snapshot.connected}
        onClick={() => void run(music.playHere)}
      >
        Play here
      </Button>
    </div>
  );
}

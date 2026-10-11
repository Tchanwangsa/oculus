import { Pause, Play, SkipBack, SkipForward } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { music, musicControllable, shownPlayback, type MusicSnapshot } from "@/lib/music";
import { useMusicStore } from "@/stores/music/musicStore";

/** Previous, play/pause and next, for what the player shows: Oculus, or the
 *  device playing elsewhere. Play and pause are explicit commands, so a
 *  click on a stale button never does the opposite. */
export function MusicTransport({
  snapshot,
  large = false,
}: {
  snapshot: MusicSnapshot;
  large?: boolean;
}) {
  const run = useMusicStore((s) => s.run);
  const disabled = !musicControllable(snapshot);
  const playing = shownPlayback(snapshot).playing;
  const icon = large ? 18 : 15;

  return (
    <div className="flex items-center justify-center gap-2">
      <Button
        variant="ghost"
        size={large ? "icon" : "icon-xs"}
        aria-label="Previous"
        disabled={disabled}
        onClick={() => run(music.prev)}
      >
        <SkipBack size={icon} weight="fill" />
      </Button>
      <Button
        size={large ? "icon-lg" : "icon"}
        aria-label={playing ? "Pause" : "Play"}
        disabled={disabled}
        onClick={() => run(playing ? music.pausePlayer : music.play)}
      >
        {playing ? (
          <Pause size={icon + 2} weight="fill" />
        ) : (
          <Play size={icon + 2} weight="fill" />
        )}
      </Button>
      <Button
        variant="ghost"
        size={large ? "icon" : "icon-xs"}
        aria-label="Next"
        disabled={disabled}
        onClick={() => run(music.next)}
      >
        <SkipForward size={icon} weight="fill" />
      </Button>
    </div>
  );
}

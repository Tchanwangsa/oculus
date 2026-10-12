import { Slider } from "@/components/ui/slider";
import { fmtClockSecs } from "@/lib/lectures/media";
import { music, shownPlayback, type MusicSnapshot } from "@/lib/music";
import { cn } from "@/lib/utils";
import { useMusicPosition } from "@/hooks/music/useMusicPosition";
import { useSliderDraft } from "@/hooks/music/useSliderDraft";
import { useMusicStore } from "@/stores/music/musicStore";
import { MUSIC_SLIDER } from "./sliderStyle";

/** The seek slider over elapsed and total of what the player shows. A drag
 *  seeks once, on release. */
export function MusicSeek({
  snapshot,
  active = true,
}: {
  snapshot: MusicSnapshot;
  /** The pane is on screen; off, the clock stops ticking. */
  active?: boolean;
}) {
  const run = useMusicStore((s) => s.run);
  const position = useMusicPosition(snapshot, active);
  const playback = shownPlayback(snapshot);
  const { draft, change, commit } = useSliderDraft(playback.position_at_ms);
  const duration = playback.track?.duration_ms ?? 0;
  const shown = draft ?? position;
  const long = duration >= 3_600_000;

  return (
    <div>
      <Slider
        min={0}
        max={Math.max(duration, 1)}
        step={1000}
        value={[Math.min(shown, duration)]}
        disabled={!snapshot.connected || !playback.track}
        onValueChange={([v]) => change(v)}
        onValueCommit={([v]) => {
          commit(v);
          run(() => music.seek(v));
        }}
        aria-label="Seek"
        className={cn("cursor-pointer py-1", MUSIC_SLIDER)}
      />
      <div className="mt-1 flex justify-between text-[11px] text-muted-foreground tabular-nums">
        <span>{fmtClockSecs(Math.floor(shown / 1000), long)}</span>
        <span>{fmtClockSecs(Math.floor(duration / 1000))}</span>
      </div>
    </div>
  );
}

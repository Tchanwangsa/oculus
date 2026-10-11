import { Pause, Play } from "@phosphor-icons/react";

import { Segmented } from "@/components/studio/style/Segmented";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import type { Playhead } from "@/hooks/studio/usePlayhead";

export const DURATIONS = [
  { value: 2, label: "2 s" },
  { value: 4, label: "4 s" },
  { value: 8, label: "8 s" },
] as const;

/** Play and scrub over the component's `t`, and how long the scene it is
 *  drawn in lasts (`ctx.duration`). */
export function TimeBar({
  head,
  duration,
  onDuration,
}: {
  head: Playhead;
  duration: number;
  onDuration: (seconds: number) => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
      <Button
        type="button"
        variant="outline"
        size="icon-sm"
        aria-label={head.playing ? "Pause" : "Play"}
        onClick={head.toggle}
      >
        {head.playing ? <Pause size={14} weight="fill" /> : <Play size={14} weight="fill" />}
      </Button>
      <Slider
        className="min-w-24 flex-1 basis-32"
        aria-label="Time in the scene"
        min={0}
        max={duration - 0.01}
        step={0.01}
        value={[head.t]}
        onValueChange={([t]) => head.seek(t)}
      />
      <span className="min-w-[72px] text-right text-[12px] tabular-nums text-muted-foreground">
        {head.t.toFixed(2)} / {duration} s
      </span>
      <div className="w-32 shrink-0">
        <Segmented label="Scene length" options={DURATIONS} value={duration} onChange={onDuration} />
      </div>
    </div>
  );
}

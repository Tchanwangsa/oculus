import type { VideoPlayback } from "@/hooks/studio/useVideoPlayback";
import { fmtClockSecs } from "@/lib/lectures/media";
import type { VideoDetail } from "@/lib/studio/videos";
import { cn } from "@/lib/utils";

/** The script split by scene: each scene's words, its length, and a press
 *  that seeks the player to it. The scene under the playhead is lit. */
export function ScriptList({ detail, playback }: { detail: VideoDetail; playback: VideoPlayback }) {
  return (
    <div className="flex flex-col gap-0.5">
      {detail.scenes.map((scene, i) => {
        const start = scene.start;
        return (
          <button
            key={i}
            type="button"
            disabled={start == null}
            onClick={() => start != null && playback.player.seek(start)}
            className={cn(
              "flex w-full cursor-pointer gap-3 rounded-lg px-3 py-2.5 text-left transition-colors disabled:cursor-default",
              playback.current === i ? "bg-brand-muted" : "enabled:hover:bg-accent",
            )}
          >
            <span className="w-4 shrink-0 pt-px text-[12px] tabular-nums text-muted-foreground">{i + 1}</span>
            <span
              className={cn(
                "min-w-0 flex-1 text-[13px] leading-relaxed",
                scene.text ? "text-foreground" : "text-muted-foreground",
              )}
            >
              {scene.text ?? `${scene.component}, no narration`}
            </span>
            {scene.duration != null && (
              <span className="shrink-0 pt-px text-[11.5px] tabular-nums text-muted-foreground">
                {fmtClockSecs(scene.duration)}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}

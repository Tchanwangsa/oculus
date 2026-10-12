import { useRef, useState } from "react";
import { WarningCircle } from "@phosphor-icons/react";

import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { useSceneStill } from "@/hooks/studio/useSceneStill";
import type { VideoPlayback } from "@/hooks/studio/useVideoPlayback";
import { fmtClockSecs } from "@/lib/lectures/media";
import type { SceneInfo, VideoDetail } from "@/lib/studio/videos";
import { cn } from "@/lib/utils";

const TILE_HEIGHT = 64;
/** Pixels wide the stills are asked for: twice the tile, for a sharp edge. */
const FRAME_WIDTH = 256;

/**
 * One tile per scene, in order, each a still from the scene (`studio_frame`),
 * lit while the playhead is in it. A press seeks to the scene's start; a
 * scene that is not narrated has none, so its tile is only a picture. A
 * still that cannot be made (a stale narration) says why once, below. The
 * thread page's strip; the detail page's dock has the grid (`SceneGrid`).
 */
export function SceneStrip({ detail, playback }: { detail: VideoDetail; playback: VideoPlayback }) {
  const ref = useRef<HTMLDivElement>(null);
  useScrollFade(ref, "x");
  const [problem, setProblem] = useState<string | null>(null);
  const aspect = detail.width / detail.height;

  return (
    <div className="min-w-0">
      <div ref={ref} className="flex gap-2 overflow-x-auto pb-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        {detail.scenes.map((scene, i) => (
          <SceneTile
            key={i}
            detail={detail}
            index={i}
            scene={scene}
            aspect={aspect}
            active={playback.current === i}
            onSeek={playback.player.seek}
            onProblem={setProblem}
          />
        ))}
      </div>
      {problem && <p className="mt-1.5 text-[11.5px] leading-snug text-muted-foreground">{problem}</p>}
    </div>
  );
}

function SceneTile({
  detail,
  index,
  scene,
  aspect,
  active,
  onSeek,
  onProblem,
}: {
  detail: VideoDetail;
  index: number;
  scene: SceneInfo;
  aspect: number;
  active: boolean;
  onSeek: (seconds: number) => void;
  onProblem: (message: string) => void;
}) {
  const still = useSceneStill(detail, index, scene, FRAME_WIDTH, onProblem);
  const start = scene.start;
  const failure = still.error;

  return (
    <button
      type="button"
      disabled={start == null}
      onClick={() => start != null && onSeek(start)}
      title={scene.text ?? scene.component}
      className="group flex shrink-0 cursor-pointer flex-col gap-1 text-left disabled:cursor-default"
    >
      <span
        style={{ height: TILE_HEIGHT, aspectRatio: aspect }}
        className={cn(
          "relative flex items-center justify-center overflow-hidden rounded-md border bg-secondary transition-[border-color,box-shadow]",
          active ? "border-brand ring-2 ring-brand/30" : "border-border group-enabled:group-hover:border-foreground/30",
        )}
      >
        {still.url ? (
          <img src={still.url} alt="" draggable={false} className="size-full object-cover" />
        ) : failure ? (
          <WarningCircle size={16} className="text-muted-foreground" />
        ) : null}
      </span>
      <span className="flex gap-1.5 px-0.5 text-[10.5px] tabular-nums text-muted-foreground">
        <span>{index + 1}</span>
        {scene.duration != null && <span>{fmtClockSecs(scene.duration)}</span>}
      </span>
    </button>
  );
}

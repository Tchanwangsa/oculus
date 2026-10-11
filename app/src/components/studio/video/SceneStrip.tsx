import { useEffect, useRef, useState } from "react";
import { WarningCircle } from "@phosphor-icons/react";

import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { useStill } from "@/hooks/studio/useStill";
import type { VideoPlayback } from "@/hooks/studio/useVideoPlayback";
import { fmtClockSecs } from "@/lib/lectures/media";
import { studioFrame, type SceneInfo, type VideoDetail } from "@/lib/studio/videos";
import { cn } from "@/lib/utils";

const TILE_HEIGHT = 64;
/** Pixels wide the stills are asked for: twice the tile, for a sharp edge. */
const FRAME_WIDTH = 256;

/**
 * One tile per scene, in order, each a still from the scene (`studio_frame`),
 * lit while the playhead is in it. A press seeks to the scene's start; a
 * scene that is not narrated has none, so its tile is only a picture. A
 * still that cannot be made (a stale narration) says why once, below.
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
            slug={detail.row.slug}
            version={detail.row.modified}
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
  slug,
  version,
  index,
  scene,
  aspect,
  active,
  onSeek,
  onProblem,
}: {
  slug: string;
  version: number | null;
  index: number;
  scene: SceneInfo;
  aspect: number;
  active: boolean;
  onSeek: (seconds: number) => void;
  onProblem: (message: string) => void;
}) {
  // Near the scene's end, where its content has arrived; a second in when
  // there is no narration to say how long it lasts.
  const t = scene.duration != null ? scene.duration * 0.8 : 1;
  const still = useStill(`frame:${slug}:${version}:${index}:${FRAME_WIDTH}`, () =>
    studioFrame(slug, { scene: index, t }, FRAME_WIDTH),
  );
  const start = scene.start;
  const failure = still.error;
  useEffect(() => {
    if (failure) onProblem(failure);
  }, [failure, onProblem]);

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

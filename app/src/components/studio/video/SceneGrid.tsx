import { useEffect, useRef, useState } from "react";
import { WarningCircle } from "@phosphor-icons/react";

import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { useSceneStill } from "@/hooks/studio/useSceneStill";
import { fmtClockSecs } from "@/lib/lectures/media";
import type { SceneInfo, VideoDetail } from "@/lib/studio/videos";
import { cn } from "@/lib/utils";

/** Pixels wide the stills are asked for: a tile is up to ~200 CSS px, drawn
 *  at 2x. */
const FRAME_WIDTH = 384;

/**
 * The Scenes tab: a still per scene in a grid that fills the dock's width,
 * the number and length under each. The scene under the playhead is lit and
 * scrolled into view when the playhead enters it, unless the pointer is on
 * the grid (the reader is looking around). A press seeks to the scene's
 * start; a scene with no narration has none and is only a picture. A still
 * that cannot be made says why once, below.
 */
export function SceneGrid({
  detail,
  current,
  seekable,
  onSeek,
  open,
}: {
  detail: VideoDetail;
  current: number;
  seekable: boolean;
  onSeek: (seconds: number) => void;
  open: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useScrollFade(ref, "y", open);
  const hovered = useRef(false);
  const [problem, setProblem] = useState<string | null>(null);
  const aspect = detail.width / detail.height;

  useEffect(() => {
    const list = ref.current;
    if (!list || !open || current < 0 || hovered.current) return;
    const card = list.querySelector(`[data-scene="${current}"]`);
    if (!card) return;
    // Scrolls the grid only: `scrollIntoView` would move the page too.
    const inset = 12;
    const above = card.getBoundingClientRect().top - list.getBoundingClientRect().top - inset;
    const below = card.getBoundingClientRect().bottom - list.getBoundingClientRect().bottom + inset;
    if (above < 0) list.scrollBy({ top: above, behavior: "smooth" });
    else if (below > 0) list.scrollBy({ top: below, behavior: "smooth" });
  }, [current, open]);

  return (
    <div
      ref={ref}
      onPointerEnter={() => (hovered.current = true)}
      onPointerLeave={() => (hovered.current = false)}
      className="min-h-0 flex-1 overflow-y-auto p-3 [--scroll-fade:40px]"
    >
      <div className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-x-2.5 gap-y-3">
        {detail.scenes.map((scene, i) => (
          <SceneCard
            key={i}
            detail={detail}
            index={i}
            scene={scene}
            aspect={aspect}
            active={current === i}
            seekable={seekable}
            onSeek={onSeek}
            onProblem={setProblem}
          />
        ))}
      </div>
      {problem && <p className="mt-3 text-[11.5px] leading-snug text-muted-foreground">{problem}</p>}
    </div>
  );
}

function SceneCard({
  detail,
  index,
  scene,
  aspect,
  active,
  seekable,
  onSeek,
  onProblem,
}: {
  detail: VideoDetail;
  index: number;
  scene: SceneInfo;
  aspect: number;
  active: boolean;
  seekable: boolean;
  onSeek: (seconds: number) => void;
  onProblem: (message: string) => void;
}) {
  const still = useSceneStill(detail, index, scene, FRAME_WIDTH, onProblem);
  const start = scene.start;

  return (
    <button
      type="button"
      data-scene={index}
      disabled={start == null || !seekable}
      onClick={() => start != null && onSeek(start)}
      title={scene.text ?? scene.component}
      className="group flex min-w-0 cursor-pointer flex-col gap-1 text-left disabled:cursor-default"
    >
      <span
        style={{ aspectRatio: aspect }}
        className={cn(
          "relative flex items-center justify-center overflow-hidden rounded-md border bg-secondary transition-[border-color,box-shadow]",
          active ? "border-brand ring-2 ring-brand/30" : "border-border group-enabled:group-hover:border-foreground/30",
        )}
      >
        {still.url ? (
          <img src={still.url} alt="" draggable={false} className="size-full object-cover" />
        ) : still.error ? (
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

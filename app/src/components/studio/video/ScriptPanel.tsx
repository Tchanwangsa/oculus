import { useCallback } from "react";

import { FollowList } from "@/components/media/FollowList";
import { fmtClockSecs } from "@/lib/lectures/media";
import type { VideoDetail } from "@/lib/studio/videos";
import { cn } from "@/lib/utils";

/** A scene's words run to a few lines; rows are measured, this only sizes the
 *  scrollbar before they are. */
const ESTIMATED_SCENE_ROW = 64;

export interface ScriptPanelProps {
  detail: VideoDetail;
  /** The scene under the playhead; -1 when none is lit. */
  current: number;
  /** A render is loaded, so a press has a video to move. */
  seekable: boolean;
  onSeek: (seconds: number) => void;
  /** The dock is open: the panel stays mounted either way and slides. */
  open: boolean;
  following: boolean;
  onScrollAway: () => void;
  onBackToLive: () => void;
}

/**
 * The Script tab: the narration split by scene, each with its length. The
 * scene under the playhead is lit and the list follows it as the lecture
 * transcript does (`FollowList`); a press seeks to the scene's start. A scene
 * with no narration says what it is instead.
 */
export function ScriptPanel({
  detail,
  current,
  seekable,
  onSeek,
  open,
  following,
  onScrollAway,
  onBackToLive,
}: ScriptPanelProps) {
  const getKey = useCallback((row: number) => row, []);
  return (
    <FollowList
      count={detail.scenes.length}
      followIdx={current}
      getItemKey={getKey}
      estimateSize={ESTIMATED_SCENE_ROW}
      open={open}
      active
      following={following}
      onScrollAway={onScrollAway}
      onBackToLive={onBackToLive}
      renderRow={(row, item, measure) => {
        const scene = detail.scenes[row];
        const start = scene.start;
        return (
          <button
            type="button"
            data-index={item.index}
            ref={measure}
            disabled={start == null || !seekable}
            onClick={() => start != null && onSeek(start)}
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              width: "100%",
              transform: `translateY(${item.start}px)`,
            }}
            className={cn(
              "flex cursor-pointer gap-2.5 rounded-lg px-2.5 py-2 text-left transition-[background-color] disabled:cursor-default",
              current === row ? "bg-brand-muted" : "enabled:hover:bg-accent",
            )}
          >
            <span className="w-4 shrink-0 pt-px text-[11.5px] tabular-nums text-muted-foreground">{row + 1}</span>
            <span
              className={cn(
                "min-w-0 flex-1 text-[12.5px] leading-relaxed",
                scene.text ? "text-foreground" : "text-muted-foreground",
              )}
            >
              {scene.text ?? `${scene.component}, no narration`}
            </span>
            {scene.duration != null && (
              <span className="shrink-0 pt-px text-[11px] tabular-nums text-muted-foreground">
                {fmtClockSecs(scene.duration)}
              </span>
            )}
          </button>
        );
      }}
    />
  );
}

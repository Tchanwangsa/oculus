import { memo } from "react";

import { MediaDock } from "@/components/media/MediaDock";
import { SceneGrid } from "@/components/studio/video/SceneGrid";
import { ScriptPanel } from "@/components/studio/video/ScriptPanel";
import type { ViewTab } from "@/components/ui/table/ViewTabs";
import type { VideoDetail } from "@/lib/studio/videos";
import type { Dock } from "@/stores/lectures/playerPrefsStore";

export type VideoDockTab = "script" | "scenes";

const TABS: ReadonlyArray<ViewTab<VideoDockTab>> = [
  { value: "script", label: "Script" },
  { value: "scenes", label: "Scenes" },
];

/** A stored tab name, tolerantly. */
export function readVideoDockTab(raw: string | null): VideoDockTab {
  return raw === "scenes" ? "scenes" : "script";
}

/**
 * The detail page's dock: the Script and the Scenes of a video in the media
 * player's `MediaDock` (side, size, drag-to-dock and fold are the player's,
 * as a lecture's). Memoised against a player that re-renders on every
 * `timeupdate`; only a scene change reaches it.
 */
export const VideoDock = memo(function VideoDock({
  detail,
  tab,
  onTabChange,
  current,
  seekable,
  onSeek,
  dock,
  size,
  open,
  resizing,
  onClose,
  onHeaderPointerDown,
  following,
  onScrollAway,
  onBackToLive,
}: {
  detail: VideoDetail;
  tab: VideoDockTab;
  onTabChange: (tab: VideoDockTab) => void;
  /** The scene under the playhead; -1 when none is lit. */
  current: number;
  /** A render is loaded, so a press has a video to move. */
  seekable: boolean;
  onSeek: (seconds: number) => void;
  dock: Dock;
  size: number;
  open: boolean;
  resizing: boolean;
  onClose: () => void;
  onHeaderPointerDown: (e: React.PointerEvent) => void;
  following: boolean;
  onScrollAway: () => void;
  onBackToLive: () => void;
}) {
  return (
    <MediaDock
      tabs={TABS}
      value={tab}
      onChange={onTabChange}
      dock={dock}
      size={size}
      open={open}
      resizing={resizing}
      onClose={onClose}
      onHeaderPointerDown={onHeaderPointerDown}
    >
      {tab === "script" ? (
        <ScriptPanel
          detail={detail}
          current={current}
          seekable={seekable}
          onSeek={onSeek}
          open={open}
          following={following}
          onScrollAway={onScrollAway}
          onBackToLive={onBackToLive}
        />
      ) : (
        <SceneGrid detail={detail} current={current} seekable={seekable} onSeek={onSeek} open={open} />
      )}
    </MediaDock>
  );
});

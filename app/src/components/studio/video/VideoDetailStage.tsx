import { RenderBar } from "@/components/studio/video/RenderBar";
import { VideoDock, readVideoDockTab } from "@/components/studio/video/VideoDock";
import { VideoFacts } from "@/components/studio/video/VideoFacts";
import { VideoPlayer } from "@/components/studio/video/VideoPlayer";
import { useStoredState } from "@/hooks/ui/useStoredState";
import type { VideoPlayback } from "@/hooks/studio/useVideoPlayback";
import type { VideoDetail } from "@/lib/studio/videos";

const TAB_KEY = "oculus-studio-video-dock-tab";

/**
 * The detail page's video: the player with the Script and Scenes in its dock,
 * and under it the facts and the render bar (no buttons: Render draft and
 * Export are in the header). Folding the dock (its button, T
 * or its ×) leaves the picture the whole width.
 */
export function VideoDetailStage({ detail, playback }: { detail: VideoDetail; playback: VideoPlayback }) {
  const [tab, setTab] = useStoredState(TAB_KEY, readVideoDockTab);
  const { player, current, src } = playback;
  const { dock, size, resizing, startDockDrag } = player.dock;

  return (
    <div className="flex min-w-0 flex-col gap-3">
      <VideoPlayer
        playback={playback}
        dockLabel={`${player.dockOpen ? "Hide" : "Show"} ${tab} (T)`}
        dock={
          <VideoDock
            detail={detail}
            tab={tab}
            onTabChange={setTab}
            current={current}
            seekable={!!src}
            onSeek={player.seek}
            dock={dock}
            size={size}
            open={player.dockOpen}
            resizing={resizing}
            onClose={player.closeDock}
            onHeaderPointerDown={startDockDrag}
            following={player.following}
            onScrollAway={player.onScrollAway}
            onBackToLive={player.onBackToLive}
          />
        }
      />
      <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
        <div className="min-w-0 flex-[3_1_20rem]">
          <VideoFacts detail={detail} />
        </div>
        <div className="min-w-0 flex-[2_1_18rem] empty:hidden">
          <RenderBar detail={detail} playback={playback} />
        </div>
      </div>
    </div>
  );
}

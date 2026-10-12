import { RenderBar } from "@/components/studio/video/RenderBar";
import { SceneStrip } from "@/components/studio/video/SceneStrip";
import { VideoPlayer } from "@/components/studio/video/VideoPlayer";
import type { VideoPlayback } from "@/hooks/studio/useVideoPlayback";
import type { VideoDetail } from "@/lib/studio/videos";

/** The player, its render bar and the scene strip: what a video looks like,
 *  on the thread page and on the detail page alike. */
export function VideoStage({ detail, playback }: { detail: VideoDetail; playback: VideoPlayback }) {
  return (
    <div className="flex min-w-0 flex-col gap-3">
      <VideoPlayer playback={playback} />
      <RenderBar detail={detail} playback={playback} />
      <SceneStrip detail={detail} playback={playback} />
    </div>
  );
}

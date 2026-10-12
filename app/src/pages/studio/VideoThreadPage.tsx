import { useCallback, useEffect } from "react";
import { Navigate, useNavigate, useParams } from "react-router-dom";

import { StudioHeader } from "@/components/studio/StudioHeader";
import { ThreadColumn } from "@/components/studio/thread/ThreadColumn";
import { VideoStage } from "@/components/studio/video/VideoStage";
import { VideoSide } from "@/components/studio/video/VideoSide";
import { Button } from "@/components/ui/button";
import { useVideoDetail } from "@/hooks/studio/useVideoDetail";
import { videoHref, VIDEOS_HREF } from "@/lib/studio/routes";
import { useVideosStore } from "@/stores/studio/videosStore";

/**
 * /studio/videos/:slug/thread: the conversation that made the video beside
 * its player, scenes and Render final. A video with no thread (made from the
 * CLI, or its thread deleted) has only its detail page.
 */
export default function VideoThreadPage() {
  const { slug = "" } = useParams();
  const navigate = useNavigate();
  const loaded = useVideosStore((s) => s.loaded);
  const row = useVideosStore((s) => s.videos.find((v) => v.slug === slug) ?? null);
  const { detail, error, reload } = useVideoDetail(slug);
  const reloadList = useVideosStore((s) => s.reload);
  const onTurnEnd = useCallback(() => {
    void reload();
    void reloadList();
  }, [reload, reloadList]);

  const threadId = row?.threadId ?? null;
  // A listed video that has no thread belongs on its detail page.
  const threadless = loaded && row != null && threadId == null;
  useEffect(() => {
    document.title = row ? `${row.title} · Oculus` : "Oculus";
    return () => {
      document.title = "Oculus";
    };
  }, [row]);

  if (threadless) return <Navigate to={videoHref(slug, row.title)} replace />;

  const title = row?.title ?? slug;
  return (
    <>
      <StudioHeader
        title={title}
        back={VIDEOS_HREF}
        actions={
          <Button
            variant="outline"
            size="sm"
            data-tab-href={videoHref(slug, title)}
            onClick={() => navigate(videoHref(slug, title))}
          >
            Open in Videos
          </Button>
        }
      />
      <div className="flex min-h-0 flex-1">
        {threadId != null && <ThreadColumn threadId={threadId} onTurnEnd={onTurnEnd} />}
        <VideoSide detail={detail} error={error} className="w-[46%] min-w-[380px] max-w-[620px] border-l border-border-subtle">
          {(d, playback) => <VideoStage detail={d} playback={playback} />}
        </VideoSide>
      </div>
    </>
  );
}

import { useEffect } from "react";
import { useNavigate, useParams } from "react-router-dom";

import { StudioHeader } from "@/components/studio/StudioHeader";
import { ExportMenu } from "@/components/studio/video/ExportMenu";
import { VideoDetailStage } from "@/components/studio/video/VideoDetailStage";
import { WithPlayback } from "@/components/studio/video/WithPlayback";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useVideoDetail } from "@/hooks/studio/useVideoDetail";
import { videoHref, VIDEOS_HREF } from "@/lib/studio/routes";
import { useVideosStore } from "@/stores/studio/videosStore";

/** The page's width: the video fills it, though not past a sane size. */
const PAGE = "mx-auto w-full max-w-[1440px] px-6 py-5";

/**
 * /studio/videos/:slug: the player with the script and scenes in its dock,
 * the facts and Render final under it, Continue in chat (the video's thread,
 * when it has one) and the Export menu of presets.
 */
export default function VideoPage() {
  const { slug = "" } = useParams();
  const navigate = useNavigate();
  const row = useVideosStore((s) => s.videos.find((v) => v.slug === slug) ?? null);
  const { detail, error } = useVideoDetail(slug);
  const title = row?.title ?? detail?.row.title ?? slug;
  const threadId = row?.threadId ?? detail?.row.threadId ?? null;

  useEffect(() => {
    document.title = `${title} · Oculus`;
    return () => {
      document.title = "Oculus";
    };
  }, [title]);

  return (
    <>
      <StudioHeader
        title={title}
        back={VIDEOS_HREF}
        actions={
          <>
            {threadId != null && (
              <Button
                variant="outline"
                size="sm"
                data-tab-href={videoHref(slug, title, true)}
                onClick={() => navigate(videoHref(slug, title, true))}
              >
                Continue in chat
              </Button>
            )}
            {detail && <ExportMenu detail={detail} />}
          </>
        }
      />
      <div className="page-scroll min-h-0 flex-1">
        {detail ? (
          <WithPlayback detail={detail} docked>
            {(playback) => (
              <div className={PAGE}>
                <VideoDetailStage detail={detail} playback={playback} />
              </div>
            )}
          </WithPlayback>
        ) : error ? (
          <p className={`${PAGE} text-[12.5px] leading-snug text-destructive`}>{error}</p>
        ) : (
          <div className={PAGE}>
            <Skeleton className="aspect-video w-full rounded-xl" />
          </div>
        )}
      </div>
    </>
  );
}

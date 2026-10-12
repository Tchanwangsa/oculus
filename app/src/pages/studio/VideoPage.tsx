import { useEffect } from "react";
import { useNavigate, useParams } from "react-router-dom";

import { StudioHeader } from "@/components/studio/StudioHeader";
import { CaptionsSwitch } from "@/components/studio/video/CaptionsSwitch";
import { ExportMenu } from "@/components/studio/video/ExportMenu";
import { ScriptList } from "@/components/studio/video/ScriptList";
import { VideoFacts } from "@/components/studio/video/VideoFacts";
import { VideoStage } from "@/components/studio/video/VideoStage";
import { WithPlayback } from "@/components/studio/video/WithPlayback";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useVideoDetail } from "@/hooks/studio/useVideoDetail";
import { videoHref, VIDEOS_HREF } from "@/lib/studio/routes";
import { useVideosStore } from "@/stores/studio/videosStore";

/**
 * /studio/videos/:slug: the player and scenes beside the script, the facts
 * and the captions switch, Continue in chat (the video's thread, when it has
 * one) and the Export menu of presets.
 */
export default function VideoPage() {
  const { slug = "" } = useParams();
  const navigate = useNavigate();
  const row = useVideosStore((s) => s.videos.find((v) => v.slug === slug) ?? null);
  const { detail, error, reload } = useVideoDetail(slug);
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
          <WithPlayback detail={detail}>
            {(playback) => (
              <div className="mx-auto flex max-w-6xl gap-8 px-6 py-5 max-lg:flex-col">
                <div className="min-w-0 flex-1">
                  <VideoStage detail={detail} playback={playback} />
                </div>
                <div className="w-full shrink-0 space-y-4 lg:w-[360px]">
                  <VideoFacts detail={detail} />
                  <CaptionsSwitch detail={detail} onChanged={() => void reload()} />
                  <div>
                    <h2 className="mb-1 px-3 text-[11px] font-medium tracking-wide text-muted-foreground">Script</h2>
                    <ScriptList detail={detail} playback={playback} />
                  </div>
                </div>
              </div>
            )}
          </WithPlayback>
        ) : error ? (
          <p className="mx-auto max-w-6xl px-6 py-5 text-[12.5px] leading-snug text-destructive">{error}</p>
        ) : (
          <div className="mx-auto max-w-6xl px-6 py-5">
            <Skeleton className="aspect-video w-full max-w-3xl rounded-xl" />
          </div>
        )}
      </div>
    </>
  );
}

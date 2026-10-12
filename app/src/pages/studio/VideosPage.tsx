import { useEffect, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { FilmSlate, Plus } from "@phosphor-icons/react";

import { StudioHeader } from "@/components/studio/StudioHeader";
import { ALL, SubjectPills } from "@/components/studio/videos/SubjectPills";
import { VideoCard } from "@/components/studio/videos/VideoCard";
import { Button } from "@/components/ui/button";
import { MediaGrid } from "@/components/ui/layout/MediaGrid";
import { EmptyState } from "@/components/ui/layout/PageParts";
import { Skeleton } from "@/components/ui/skeleton";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { NEW_VIDEO_HREF } from "@/lib/studio/routes";
import { useVideosStore } from "@/stores/studio/videosStore";

/** /studio/videos: every video as a thumbnail, narrowed by subject. */
export default function VideosPage() {
  const navigate = useNavigate();
  const { videos, loaded, error } = useVideosStore();
  const [chosen, setChosen] = useStoredState("oculus-studio-subject", (raw) => raw ?? ALL);

  const subjects = useMemo(() => [...new Set(videos.flatMap((v) => (v.subject ? [v.subject] : [])))], [videos]);
  // A remembered subject that no longer has a video shows everything.
  const filter = subjects.includes(chosen) ? chosen : ALL;
  const shown = filter === ALL ? videos : videos.filter((v) => v.subject === filter);
  const newVideo = () => navigate(NEW_VIDEO_HREF);
  // An agent or the CLI may have changed the folders since the layout read them.
  useEffect(() => {
    void useVideosStore.getState().reload();
  }, []);

  return (
    <>
      <StudioHeader
        title="Videos"
        actions={
          <Button size="sm" data-tab-href={NEW_VIDEO_HREF} onClick={newVideo}>
            <Plus size={14} weight="bold" />
            New video
          </Button>
        }
      />
      <div className="page-scroll min-h-0 flex-1">
        <div className="mx-auto max-w-5xl space-y-5 px-6 py-5">
          <SubjectPills subjects={subjects} value={filter} onChange={setChosen} />
          {!loaded ? (
            <MediaGrid>
              {Array.from({ length: 6 }, (_, i) => (
                <Skeleton key={i} className="aspect-video w-full rounded-xl" />
              ))}
            </MediaGrid>
          ) : error ? (
            <p className="text-[12.5px] leading-snug text-destructive">{error}</p>
          ) : videos.length === 0 ? (
            <EmptyState
              icon={<FilmSlate size={28} weight="light" className="text-muted-foreground" />}
              title="No videos yet"
              body="Describe one and an agent scripts, narrates and renders it."
            >
              <Button size="sm" onClick={newVideo}>
                New video
              </Button>
            </EmptyState>
          ) : (
            <MediaGrid>
              {shown.map((v) => (
                <VideoCard key={v.slug} video={v} />
              ))}
            </MediaGrid>
          )}
        </div>
      </div>
    </>
  );
}

import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { CircleNotch, FilmSlate } from "@phosphor-icons/react";

import { MediaTile } from "@/components/ui/layout/MediaGrid";
import { useStill } from "@/hooks/studio/useStill";
import { displayCode, fmtAgo } from "@/lib/format/format";
import { videoHref } from "@/lib/studio/routes";
import { studioThumbnail, type VideoRow } from "@/lib/studio/videos";
import { useRenderStore } from "@/stores/studio/renderStore";

/** A video in the grid, opening its detail: its thumbnail (a still from scene 0), title and
 *  facts; while it renders, a badge and a bar with the percentage. */
export function VideoCard({ video }: { video: VideoRow }) {
  const navigate = useNavigate();
  const active = useRenderStore((s) => s.active[video.dir]);
  const still = useStill(`thumb:${video.slug}:${video.modified}`, () => studioThumbnail(video.slug));
  // A portrait video's still would be cropped to the frame; show it whole.
  const [portrait, setPortrait] = useState(false);
  const href = videoHref(video.slug, video.title);
  const pct = active?.fraction != null ? Math.round(active.fraction * 100) : null;

  const facts = [
    video.subject ? displayCode(video.subject) : null,
    video.scenes != null ? `${video.scenes} ${video.scenes === 1 ? "scene" : "scenes"}` : null,
    video.modified != null ? fmtAgo(video.modified * 1000) : null,
  ].filter(Boolean);

  return (
    <MediaTile
      href={href}
      onOpen={() => navigate(href)}
      title={video.title}
      progress={active ? (active.fraction ?? 0) : null}
      badge={
        active && (
          <span className="flex items-center gap-1 rounded-full bg-card/95 px-2 py-0.5 text-[11px] font-medium text-foreground shadow-sm">
            <CircleNotch size={11} className="animate-spin text-brand" />
            Rendering{pct != null && <span className="tabular-nums"> {pct}%</span>}
          </span>
        )
      }
      meta={
        video.problem ? (
          <span className="text-destructive" title={video.problem}>
            {video.problem}
          </span>
        ) : (
          facts.join(" · ")
        )
      }
      picture={
        still.url ? (
          <img
            src={still.url}
            alt=""
            draggable={false}
            onLoad={(e) => setPortrait(e.currentTarget.naturalHeight > e.currentTarget.naturalWidth)}
            className={portrait ? "size-full bg-black object-contain" : "size-full object-cover"}
          />
        ) : (
          <span className="flex size-full items-center justify-center text-muted-foreground" title={still.error ?? undefined}>
            <FilmSlate size={22} weight="light" />
          </span>
        )
      }
    />
  );
}

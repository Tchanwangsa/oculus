import { NavLink, useLocation } from "react-router-dom";
import { CircleNotch } from "@phosphor-icons/react";

import {
  SIDE_NAV_FOLDS,
  SIDE_NAV_ROW,
  SIDE_NAV_ROW_ACTIVE,
  SIDE_NAV_ROW_IDLE,
} from "@/components/ui/layout/SideNav";
import { displayCode } from "@/lib/format/format";
import { VIDEOS_HREF, videoHref } from "@/lib/studio/routes";
import type { VideoRow } from "@/lib/studio/videos";
import { cn } from "@/lib/utils";

/** A video as a nav row: its title, then its subject, or a spinner while it
 *  renders or its thread is mid-turn. It opens the thread when there is one,
 *  else the detail; either page lights it. */
export function RecentVideoRow({
  video,
  busy,
  onOpened,
}: {
  video: VideoRow;
  busy: boolean;
  onOpened: () => void;
}) {
  const { pathname } = useLocation();
  const here = `${VIDEOS_HREF}/${encodeURIComponent(video.slug)}`;
  const active = pathname === here || pathname === `${here}/thread`;
  return (
    <NavLink
      to={videoHref(video.slug, video.title, video.threadId != null)}
      onClick={onOpened}
      title={video.title}
      className={cn(SIDE_NAV_ROW, active ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE)}
    >
      <span className={cn("min-w-0 flex-1 truncate", SIDE_NAV_FOLDS)}>{video.title}</span>
      {busy ? (
        <CircleNotch size={12} className="shrink-0 animate-spin text-brand" aria-label="Working" />
      ) : (
        video.subject && (
          <span className={cn("shrink-0 text-[10.5px] tabular-nums text-muted-foreground", SIDE_NAV_FOLDS)}>
            {displayCode(video.subject)}
          </span>
        )
      )}
    </NavLink>
  );
}

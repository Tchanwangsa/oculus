import type { ReactNode } from "react";

import { useVideoPlayback, type VideoPlayback } from "@/hooks/studio/useVideoPlayback";
import type { VideoDetail } from "@/lib/studio/videos";

/** Gives `children` the player for a loaded video: the hook needs a detail,
 *  so it lives below the page's loading and error states. `docked` is for a
 *  page that gives the player a dock. */
export function WithPlayback({
  detail,
  docked = false,
  children,
}: {
  detail: VideoDetail;
  docked?: boolean;
  children: (playback: VideoPlayback) => ReactNode;
}) {
  return <>{children(useVideoPlayback(detail, docked))}</>;
}

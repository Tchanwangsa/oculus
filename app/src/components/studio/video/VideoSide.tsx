import type { ReactNode } from "react";

import { WithPlayback } from "@/components/studio/video/WithPlayback";
import { Skeleton } from "@/components/ui/skeleton";
import type { VideoPlayback } from "@/hooks/studio/useVideoPlayback";
import type { VideoDetail } from "@/lib/studio/videos";
import { cn } from "@/lib/utils";

/** The thread page's right column: the video's detail while it loads, why it
 *  could not load, or `children` for the loaded video. */
export function VideoSide({
  detail,
  error,
  className,
  children,
}: {
  detail: VideoDetail | null;
  error: string | null;
  className?: string;
  children: (detail: VideoDetail, playback: VideoPlayback) => ReactNode;
}) {
  return (
    <aside className={cn("shrink-0 overflow-y-auto p-5", className)}>
      {detail ? (
        <WithPlayback detail={detail}>{(playback) => children(detail, playback)}</WithPlayback>
      ) : error ? (
        <p className="text-[12.5px] leading-snug text-destructive">{error}</p>
      ) : (
        <div className="space-y-3">
          <Skeleton className="aspect-video w-full rounded-xl" />
          <Skeleton className="h-8 w-full" />
        </div>
      )}
    </aside>
  );
}

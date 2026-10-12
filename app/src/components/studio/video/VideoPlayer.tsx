import type { ReactNode } from "react";
import { FilmSlate } from "@phosphor-icons/react";

import { MediaPlayer } from "@/components/media/MediaPlayer";
import { useDockedFrameHeight } from "@/hooks/studio/useDockedFrameHeight";
import type { VideoPlayback } from "@/hooks/studio/useVideoPlayback";
import { cn } from "@/lib/utils";

const FRAME = "overflow-hidden rounded-xl border border-border bg-black";

/**
 * The render in the media player, in a 16:9 picture whatever the video's
 * shape (a portrait video letterboxes), with a captions button when the
 * render's captions are beside it. With no render yet the picture says so;
 * the render bar beside it offers one.
 *
 * With a `dock` (the detail page's, labelled `dockLabel` on the bar) the
 * frame holds the picture and the dock, and takes the height that keeps the
 * picture 16:9 beside it (`useDockedFrameHeight`), within the window. Without
 * one (the thread page) the frame is the picture alone.
 */
export function VideoPlayer({
  playback,
  dock,
  dockLabel,
}: {
  playback: VideoPlayback;
  dock?: ReactNode;
  dockLabel?: string;
}) {
  const { player, shown, src, setEl, chapters, hasCaptions } = playback;
  const frame = useDockedFrameHeight(player);

  if (!dock && !shown) {
    return (
      <div className={cn("flex aspect-video w-full", FRAME)}>
        <NotRendered />
      </div>
    );
  }

  const docked = dock != null;
  return (
    <div
      ref={frame.ref}
      style={docked && frame.height != null ? { height: frame.height } : undefined}
      className={cn(
        "flex w-full",
        FRAME,
        !docked && "aspect-video",
        // Within the window, though never squeezed to nothing.
        docked && "max-h-[calc(100vh-14rem)] min-h-[18rem]",
        docked && frame.eases && "transition-[height] duration-200 ease-out",
      )}
    >
      <MediaPlayer
        player={player}
        canPlay={!!src}
        previewSrc={src}
        chapters={chapters}
        captions={hasCaptions}
        dock={dock}
        dockLabel={dockLabel}
        // Under the control bar, so the dock button still folds and opens it.
        overlay={
          !shown && (
            <div className="absolute inset-0 z-20 flex bg-black">
              <NotRendered />
            </div>
          )
        }
      >
        <div className="relative min-h-0 min-w-0 flex-1 overflow-hidden">
          {src && (
            <video
              ref={setEl}
              src={src}
              playsInline
              preload="metadata"
              className="h-full w-full object-contain"
            />
          )}
        </div>
      </MediaPlayer>
    </div>
  );
}

function NotRendered() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 text-center text-white/80">
      <FilmSlate size={28} weight="light" />
      <p className="text-[13px]">Not rendered yet</p>
    </div>
  );
}

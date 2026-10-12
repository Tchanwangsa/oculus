import { FilmSlate } from "@phosphor-icons/react";

import { MediaPlayer } from "@/components/media/MediaPlayer";
import type { VideoPlayback } from "@/hooks/studio/useVideoPlayback";

/**
 * The render in the media player, in a 16:9 frame whatever the video's shape
 * (a portrait video letterboxes). With no render yet the frame says so; the
 * render bar beside it offers one.
 */
export function VideoPlayer({ playback }: { playback: VideoPlayback }) {
  const { player, shown, src, setEl, chapters } = playback;
  return (
    <div className="flex aspect-video w-full overflow-hidden rounded-xl border border-border bg-black">
      {shown ? (
        <MediaPlayer
          player={player}
          canPlay={!!src}
          previewSrc={src}
          chapters={chapters}
          captions={false}
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
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 text-center text-white/80">
          <FilmSlate size={28} weight="light" />
          <p className="text-[13px]">Not rendered yet</p>
        </div>
      )}
    </div>
  );
}

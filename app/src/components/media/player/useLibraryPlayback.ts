import { useEffect } from "react";
import { pauseLibraryVideos, registerLibraryVideo } from "@/lib/lectures/media";
import { pauseLecturePlayback } from "@/lib/lectures/playback";

/**
 * What a library video's own `<video>` needs to share the app's one sound:
 * it pauses with its tab (nothing else stops it, as a lecture's parked
 * element is), playing it pauses the lecture and any other library video,
 * and a lecture starting pauses it (`lib/lectures/playback/`).
 */
export function useLibraryPlayback(el: HTMLVideoElement | null, onScreen: boolean) {
  useEffect(() => {
    if (!onScreen) el?.pause();
  }, [onScreen, el]);

  useEffect(() => {
    if (!el) return;
    const unregister = registerLibraryVideo(el);
    const onPlay = () => {
      pauseLecturePlayback();
      pauseLibraryVideos(el);
    };
    el.addEventListener("play", onPlay);
    return () => {
      el.removeEventListener("play", onPlay);
      unregister();
    };
  }, [el]);
}

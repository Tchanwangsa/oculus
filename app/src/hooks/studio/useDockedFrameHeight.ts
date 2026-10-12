import { useEffect, useRef, useState } from "react";

import type { MediaPlayerState } from "@/components/media/MediaPlayer";
import { isVertical } from "@/stores/lectures/playerPrefsStore";

/** The picture is always 16:9, whatever the video's shape. */
const VIDEO_ASPECT = 16 / 9;
/** The dock's slide (`MediaDock`), which the frame's height eases with. */
const SLIDE_MS = 200;

/**
 * A ref for the player's frame and the height that keeps the picture 16:9
 * beside or above its dock: the frame's width less a side dock, over the
 * aspect, plus a bottom or top dock. Folding the dock gives the picture the
 * whole width. `eases` is true for the length of the dock's slide after it
 * opens or folds, so the height moves with the dock; a window resize or a
 * resize drag follows at once. `height` is null until the frame is measured.
 */
export function useDockedFrameHeight(player: MediaPlayerState) {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setWidth(el.clientWidth);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const { dockOpen } = player;
  const { dock, size } = player.dock;
  const [eases, setEases] = useState(false);
  const wasOpen = useRef(dockOpen);
  useEffect(() => {
    if (wasOpen.current === dockOpen) return;
    wasOpen.current = dockOpen;
    setEases(true);
    const timer = setTimeout(() => setEases(false), SLIDE_MS + 50);
    return () => clearTimeout(timer);
  }, [dockOpen]);

  // The dock's box plus its 1px divider.
  const span = dockOpen ? size + 1 : 0;
  const vertical = isVertical(dock);
  const height =
    width > 0 ? ((vertical ? width : width - span) / VIDEO_ASPECT) + (vertical ? span : 0) : null;
  return { ref, height, eases };
}

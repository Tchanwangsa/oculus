import { useEffect, type RefObject } from "react";
import { syncScrollFade, type ScrollFadeAxis } from "@/lib/ui/scrollFade";

/**
 * Fade each edge of a scroller that has more content behind it. A mask, so it
 * needs no background colour and works on any surface. Re-measures on scroll
 * and when the box or its first child resizes (the observer catches content
 * growing, which a scroll event misses). `live` re-hangs the listeners when
 * the scroller element is swapped, like `useStickToBottom`.
 */
export function useScrollFade(
  ref: RefObject<HTMLElement | null>,
  axis: ScrollFadeAxis = "y",
  live?: unknown,
) {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const sync = () => syncScrollFade(el, axis);
    sync();
    el.addEventListener("scroll", sync, { passive: true });
    const ro = new ResizeObserver(sync);
    ro.observe(el);
    if (el.firstElementChild) ro.observe(el.firstElementChild);
    return () => {
      el.removeEventListener("scroll", sync);
      ro.disconnect();
    };
  }, [ref, axis, live]);
}

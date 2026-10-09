import { useEffect, useRef } from "react";

/** How close to the bottom still counts as reading the bottom. */
const STICK_PX = 80;

/**
 * Follow a growing timeline, but only while the reader is at the bottom.
 * Watches both boxes: content grows, and the scroller shrinks as the composer
 * wraps. `threadId` resets the pin; `live` re-hangs the observer when the
 * scroller element is swapped.
 */
export function useStickToBottom(threadId: number | null, live: boolean) {
  const outer = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);

  useEffect(() => {
    pinned.current = true;
  }, [threadId]);

  useEffect(() => {
    const el = outer.current;
    const content = inner.current;
    if (!el || !content) return;
    const onScroll = () => {
      pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < STICK_PX;
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    const ro = new ResizeObserver(() => {
      if (pinned.current) el.scrollTop = el.scrollHeight;
    });
    ro.observe(content);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", onScroll);
      ro.disconnect();
    };
  }, [live]);

  return { outer, inner };
}

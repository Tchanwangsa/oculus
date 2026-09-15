import { useEffect, useRef } from "react";

/** How close to the bottom still counts as reading the bottom. */
const STICK_PX = 80;

/**
 * Follow the stream, but only from the bottom.
 *
 * A `scrollTo` on every delta forced a layout of the whole thread per token
 * *and* yanked the view back down whenever the reader scrolled up mid-turn;
 * growth is watched instead, and the scroll only happens while the bottom is
 * where they already are.
 *
 * Shared because the Chat page and the lecture player's dock draw the same
 * timeline (`docs/harness.md`), and two copies of this would be two answers to
 * "did the reader scroll away" — the one thing the timeline gets wrong most
 * visibly.
 *
 * `threadId` is what resets the pin: a thread you have just opened starts at
 * its end, whatever the last one was scrolled to. `live` re-hangs the observer
 * when the scroller itself is swapped out — an empty thread draws a different
 * box from a thread with rows in it.
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
    return () => {
      el.removeEventListener("scroll", onScroll);
      ro.disconnect();
    };
  }, [live]);

  return { outer, inner };
}

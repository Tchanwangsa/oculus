import { useEffect, useLayoutEffect, useRef } from "react";
import { anchorDrift, HELD_ANCHOR, pickAnchor, SKIPPED_CONTENT, type Anchor } from "@/lib/ui/scrollAnchor";

/** How close to the bottom still counts as reading the bottom. */
const STICK_PX = 80;

/** Marks a top-level row under `inner` that the view may anchor to. A class,
 *  not an attribute, so the rows are one live collection rather than a
 *  selector query per scroll event. */
export const ANCHOR_ROW = "scroll-anchor-row";

/**
 * Follow a growing timeline, but only while the reader is at the bottom.
 * Watches both boxes: content grows, and the scroller shrinks as the composer
 * wraps. Off the bottom, the row being read (an `ANCHOR_ROW`, see
 * `app/src/lib/ui/scrollAnchor.ts`) keeps its place on screen when rows above
 * it re-wrap, grow or arrive. `threadId` resets the pin; `live` re-hangs the
 * observer when the scroller element is swapped.
 */
export function useStickToBottom(threadId: number | null, live: boolean) {
  const outer = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLDivElement>(null);
  const s = useRef({
    pinned: true,
    anchor: null as Anchor | null,
    held: null as Element | null,
    /** The `scrollTop` that `pinned` and `anchor` were taken at. */
    at: Number.NaN,
  }).current;

  // Before the new thread's first resize, which must find it pinned.
  useLayoutEffect(() => {
    s.pinned = true;
    s.anchor = null;
    s.held?.removeAttribute(HELD_ANCHOR);
    s.held = null;
  }, [threadId, s]);

  useEffect(() => {
    const el = outer.current;
    const content = inner.current;
    if (!el || !content) return;
    const rows = content.getElementsByClassName(ANCHOR_ROW);
    const remember = (anchor: Anchor | null) => {
      s.anchor = anchor;
      const row = anchor?.node.closest(`.${ANCHOR_ROW}`) ?? null;
      if (s.held === row) return;
      s.held?.removeAttribute(HELD_ANCHOR);
      s.held = row;
      row?.setAttribute(HELD_ANCHOR, "");
    };
    const onScroll = () => {
      // Our own writes land here a frame later, when the next resize may have
      // dirtied layout; measuring then would anchor to the changed layout.
      if (Math.abs(el.scrollTop - s.at) < 1) return;
      s.pinned = el.scrollHeight - el.scrollTop - el.clientHeight < STICK_PX;
      s.at = el.scrollTop;
      remember(s.pinned ? null : pickAnchor(el, rows));
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    // After layout, before paint: a correction here never shows the jump.
    const ro = new ResizeObserver(() => {
      if (s.pinned) {
        el.scrollTop = el.scrollHeight;
      } else {
        const drift = s.anchor && anchorDrift(el, s.anchor, content);
        if (drift == null) remember(pickAnchor(el, rows));
        else if (Math.abs(drift) >= 0.5) el.scrollTop += drift;
        // A scroll can first encounter a placeholder. Once its host returns,
        // anchor the paragraph now being read rather than the whole row.
        if (s.anchor?.deferred && !s.anchor.node.hasAttribute(SKIPPED_CONTENT)) remember(pickAnchor(el, rows));
        else if (drift != null && Math.abs(drift) < 0.5) return;
      }
      s.at = el.scrollTop;
    });
    ro.observe(content);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", onScroll);
      ro.disconnect();
      s.held?.removeAttribute(HELD_ANCHOR);
      s.held = null;
    };
  }, [live, s]);

  return { outer, inner };
}

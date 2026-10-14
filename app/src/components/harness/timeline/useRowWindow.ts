import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { ANCHOR_ROW } from "@/hooks/ui/useStickToBottom";
import { onExpand } from "@/lib/ui/expandable";
import { scrollParent, zoomScale } from "@/lib/ui/scrollAnchor";
import type { ViewRow } from "./rows";
import { includeId, pageEarlier, settleFrom, windowStart, type From } from "./rowWindow";

/**
 * Mounts the newest rows of a thread (`rowWindow.ts`) and earlier pages as a
 * sentinel above them nears the scroller's top; `useStickToBottom` holds the
 * view still as they land. Keyed by thread: a tab's `Timeline` outlives a
 * thread switch. `rootRef` takes `expandWithin` requests (find, Select All, a
 * rail jump), and once expanded a thread stays expanded.
 */
export function useRowWindow(rows: ViewRow[], threadId: number | null) {
  const [win, setWin] = useState<{ thread: number | null; from: From | undefined }>({
    thread: threadId,
    from: undefined,
  });
  const stored = win.thread === threadId ? win.from : undefined;
  const from = settleFrom(rows, stored);
  // Adjusting state while rendering: React re-renders before committing.
  if (win.thread !== threadId || from !== stored) setWin({ thread: threadId, from });
  const start = windowStart(rows, from);

  const rootRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const latest = useRef({ rows, from, threadId });
  useLayoutEffect(() => {
    latest.current = { rows, from, threadId };
  });

  // Re-hung per boundary: an observer reports once on observe, so a sentinel
  // still in reach after a short page loads the next one.
  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel) return;
    const io = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        const { rows, from, threadId } = latest.current;
        if (from == null) return;
        setWin({ thread: threadId, from: pageEarlier(rows, from) });
      },
      // A viewport ahead, so the next page lands before the reader gets there.
      { root: scrollParent(sentinel), rootMargin: "100% 0px 0px 0px" },
    );
    io.observe(sentinel);
    return () => io.disconnect();
  }, [start, threadId]);

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    return onExpand(root, ({ sync, upToId }) => {
      const { rows, from, threadId } = latest.current;
      if (from == null) return;
      const next = upToId == null ? null : includeId(rows, from, upToId);
      if (next === from) return;
      const apply = () => setWin({ thread: threadId, from: next });
      if (!sync) return apply();
      // The caller reads layout straight after, before the anchoring observer
      // runs, so hold the first mounted row in place here.
      const scroller = scrollParent(root);
      const first = root.getElementsByClassName(ANCHOR_ROW)[0];
      const before = first?.getBoundingClientRect().top;
      flushSync(apply);
      if (!scroller || !first?.isConnected || before == null) return;
      const view = scroller.getBoundingClientRect();
      const scale = zoomScale(view.height, scroller.offsetHeight);
      scroller.scrollTop += (first.getBoundingClientRect().top - before) / scale;
    });
  }, []);

  return { start, rootRef, sentinelRef };
}

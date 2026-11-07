import { useEffect, useState, type RefObject } from "react";
import { scrollParent } from "@/lib/ui/scrollAnchor";

/** How far outside the chat's viewport a frame still counts as in view, as
 *  the timeline's row window does, so a scene is drawing by the time it shows. */
const MARGIN = "200px 0px";

/** How long a frame must stay intersecting before it counts as in view:
 *  frames moving while a thread lays out, or passed in a fast scroll, must
 *  not each create a WebGL context. */
export const IN_VIEW_SETTLE_MS = 150;

/**
 * Turns raw intersection reports into the in-view flag: leaving counts at
 * once, entering only after `IN_VIEW_SETTLE_MS` of staying in. `cancel`
 * drops a pending entry. The timer functions are parameters so it can be
 * tested without a DOM.
 */
export function settleInView(
  set: (inView: boolean) => void,
  schedule: (fn: () => void, ms: number) => unknown = setTimeout,
  unschedule: (handle: unknown) => void = (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
) {
  let pending: unknown = null;
  const cancel = () => {
    if (pending != null) unschedule(pending);
    pending = null;
  };
  return {
    update(intersecting: boolean) {
      if (!intersecting) {
        cancel();
        set(false);
      } else if (pending == null) {
        pending = schedule(() => {
          pending = null;
          set(true);
        }, IN_VIEW_SETTLE_MS);
      }
    },
    cancel,
  };
}

/**
 * Whether the frame is within `MARGIN` of its scroll parent's viewport, has
 * a height, and has stayed so for `IN_VIEW_SETTLE_MS` (leaving counts at
 * once). An observer doesn't call back when a fully visible target only
 * grows, so the frame is re-observed whenever its height changes, for an
 * entry with its real geometry. The observer is hung on the element `frame`
 * holds when `element` changes: the
 * iframe is a new element per document (`key` in `LiveWidget`), so pass the
 * document's id. Off-screen rows keep their frames attached
 * (`useRowVisibility`), so this is what tells a scene it may let go.
 */
export function useFrameInView(frame: RefObject<HTMLIFrameElement | null>, element: unknown): boolean {
  const [inView, setInView] = useState(false);
  useEffect(() => {
    const el = frame.current;
    if (!el) {
      setInView(false);
      return;
    }
    const settle = settleInView(setInView);
    const observer = new IntersectionObserver(
      (entries) => {
        const entry = entries[entries.length - 1];
        // Zero height is out: nothing to draw, and frames stacked before their first real height all intersect.
        if (entry) settle.update(entry.isIntersecting && entry.boundingClientRect.height > 0);
      },
      { root: scrollParent(el), rootMargin: MARGIN },
    );
    observer.observe(el);
    let height = -1;
    const resize = new ResizeObserver(() => {
      if (el.offsetHeight === height) return;
      height = el.offsetHeight;
      observer.unobserve(el);
      observer.observe(el);
    });
    resize.observe(el);
    return () => {
      observer.disconnect();
      resize.disconnect();
      settle.cancel();
    };
  }, [frame, element]);
  return inView;
}

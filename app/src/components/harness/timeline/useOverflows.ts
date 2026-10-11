import { useLayoutEffect, useRef, useState } from "react";

/** How much of a long question is left showing when it is folded. */
export const QUESTION_MAX_H = 208;
/** Below this much hidden, folding would save a line and cost a click. */
const FOLD_SLACK = 40;

/** Whether a bubble is long enough to fold — measured, since line count
 *  depends on the column's width. */
export function useOverflows(text: string, shown: boolean) {
  const ref = useRef<HTMLDivElement>(null);
  const [over, setOver] = useState(false);
  useLayoutEffect(() => {
    const el = ref.current;
    // While editing the node is gone; the observer re-hangs when it returns.
    if (!el || !shown) return;
    // `scrollHeight` is the full text even while `max-height` is clipping it.
    const measure = () => setOver(el.scrollHeight > QUESTION_MAX_H + FOLD_SLACK);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [text, shown]);
  return [ref, over] as const;
}

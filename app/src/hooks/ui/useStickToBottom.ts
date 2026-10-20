import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { followTimeline } from "@/lib/ui/followTimeline";

export { ANCHOR_ROW } from "@/lib/ui/followTimeline";

/** Follow active output until the reader moves up; preserve their reading
 *  position through layout changes. Each thread opens at its latest row. */
export function useStickToBottom(threadId: number | null, live: boolean, running: boolean) {
  const outer = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLDivElement>(null);
  const active = useRef(running);
  const control = useRef<ReturnType<typeof followTimeline> | null>(null);
  const [showJump, setShowJump] = useState(false);

  useLayoutEffect(() => { active.current = running; }, [running]);
  useLayoutEffect(() => {
    setShowJump(false);
    const el = outer.current;
    const content = inner.current;
    if (!live || !el || !content) return;
    const follow = followTimeline(el, content, () => active.current, setShowJump);
    control.current = follow;
    return () => {
      control.current = null;
      follow.destroy();
    };
  }, [threadId, live]);

  const scrollToBottom = useCallback(() => control.current?.scrollToBottom(), []);
  return { outer, inner, showJump, scrollToBottom };
}

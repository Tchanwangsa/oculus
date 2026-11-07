import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { followTimeline } from "@/lib/ui/followTimeline";
import type { HarnessItem } from "@/lib/harness";

export { ANCHOR_ROW } from "@/lib/ui/followTimeline";

/** Follow active output until the reader moves up; preserve their reading
 *  position through layout changes. Each thread opens at its latest row, and
 *  a new question (the reader's own send) brings the view back down. */
export function useStickToBottom(threadId: number | null, live: boolean, running: boolean, items: HarnessItem[]) {
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

  const asked = lastQuestion(items);
  const seen = useRef({ threadId, asked });
  useLayoutEffect(() => {
    const before = seen.current;
    seen.current = { threadId, asked };
    if (before.threadId === threadId && asked != null && asked !== before.asked) control.current?.scrollToBottom();
  }, [threadId, asked]);

  const scrollToBottom = useCallback(() => control.current?.scrollToBottom(), []);
  return { outer, inner, showJump, scrollToBottom };
}

function lastQuestion(items: HarnessItem[]): number | null {
  for (let i = items.length - 1; i >= 0; i--) if (items[i].kind === "user") return items[i].id;
  return null;
}

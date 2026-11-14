import { useEffect, useState } from "react";
import { useTabActive } from "@/components/tabs/TabContext";
import { threadUnread } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { markThreadViewed } from "@/stores/chat/threadViewed";

/** `document.visibilityState`, as state. */
function useDocumentVisible(): boolean {
  const [visible, setVisible] = useState(() => document.visibilityState !== "hidden");
  useEffect(() => {
    const on = () => setVisible(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", on);
    return () => document.removeEventListener("visibilitychange", on);
  }, []);
  return visible;
}

/**
 * Marks the thread a view shows as read while it is on screen: its tab in
 * front and the window visible. It writes only when a run has ended since the
 * last look, so a run settling here is read at once and one that settled in
 * another tab is read when the student comes back.
 */
export function useThreadViewed(threadId: number | null) {
  const active = useTabActive();
  const visible = useDocumentVisible();
  const unread = useHarnessStore((s) => {
    const t = threadId == null ? undefined : s.threads.find((x) => x.id === threadId);
    return !!t && threadUnread(t);
  });
  useEffect(() => {
    if (threadId != null && unread && active && visible) void markThreadViewed(threadId);
  }, [threadId, unread, active, visible]);
}

import { useEffect, useRef } from "react";
import { useTabActive } from "@/components/tabs/TabContext";
import { useWindowEvent } from "@/hooks/backend/useEvents";

/**
 * Read on mount, on the tab's return to front, and on the given window events.
 * Tabs stay mounted (`TabContext.tsx`), so without the front-edge read a
 * backgrounded Home would go stale. The edge, not `active`, avoids a double
 * first read.
 * `read` must be stable (`useCallback`), or it re-reads every render.
 */
export function useHomeSection(read: () => void, events: readonly string[]): void {
  const active = useTabActive();
  const wasActive = useRef(active);

  useEffect(() => {
    read();
  }, [read]);

  useEffect(() => {
    if (active && !wasActive.current) read();
    wasActive.current = active;
  }, [active, read]);

  useWindowEvent(events, read);
}

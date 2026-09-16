import { useEffect, useRef } from "react";
import { useTabActive } from "@/components/tabs/TabContext";

/**
 * Read now, read again when this tab comes back to the front, and read on a
 * fixed list of window events.
 *
 * Every tab stays mounted (`app/src/components/tabs/TabContext.tsx`), so a
 * section that reads once on mount is not reading once per visit — it is
 * reading once per *session*. Home is the page that suffers most from that:
 * every section on it is a recency read, and Home is the tab you leave open
 * and come back to, so a Home left in the background for an hour would show an
 * hour-old Continue list, an hour-old sync line and a day that has since
 * moved on, unless some unrelated write happened to fire an event first. The
 * front edge is the visit, so the front edge is when to re-read.
 *
 * The edge and not the raw `active` value: a tab that is already in front when
 * it mounts would otherwise read twice in a row for the same first look.
 *
 * @param read  What to re-read. Must be stable across renders — wrap it in
 *              `useCallback` with an empty dependency list — or the mount read
 *              fires again on every render.
 * @param events Window event names to also read on. Must be a stable reference:
 *              declare it as a module-level `const` in the section, never as an
 *              array literal in the call, or the subscribe effect tears down
 *              and rebuilds its listeners on every render.
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

  useEffect(() => {
    for (const name of events) window.addEventListener(name, read);
    return () => {
      for (const name of events) window.removeEventListener(name, read);
    };
  }, [events, read]);
}

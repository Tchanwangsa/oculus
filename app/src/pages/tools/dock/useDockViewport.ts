import { useEffect, useRef } from "react";
import { dock, type DockServiceId } from "@/lib/dock";
import type { NativeHole } from "@/lib/shell/nativeHoles";
import { useNativeHole } from "@/hooks/shell/useNativeHole";
import { measure } from "@/pages/tools/browser/viewport";

/** The pane that last placed each service, and its hole. Two panes can route
 *  to one service; only the one showing it may move or park it, so an
 *  outgoing pane's park can't land after the incoming pane's place. */
const holders = new Map<DockServiceId, { paneId: number; hole: NativeHole }>();

function release(service: DockServiceId, paneId: number) {
  const holder = holders.get(service);
  if (holder?.paneId !== paneId) return;
  holder.hole.close();
  holders.delete(service);
  dock.park(service).catch(() => {});
}

/**
 * Where a dock page sits: placed under the slot while its tab is in front,
 * with a hole cut over it once Rust has placed it; parked otherwise, the hole
 * closed first. Parking moves the page off-screen and keeps it running; the
 * browser's hook hides instead, which throttles timers.
 */
export function useDockViewport(service: DockServiceId, paneId: number, active: boolean) {
  const slotRef = useRef<HTMLDivElement>(null);
  const hole = useNativeHole(slotRef);
  const parked = !active;

  useEffect(() => {
    const slot = slotRef.current;
    if (!slot) return;
    if (parked) {
      release(service, paneId);
      return;
    }
    // The page leaves the other pane's slot, so its hole goes first.
    const previous = holders.get(service);
    if (previous && previous.paneId !== paneId) previous.hole.close();
    holders.set(service, { paneId, hole });
    let live = true;
    dock.place(service, measure(slot)).then(
      () => live && holders.get(service)?.paneId === paneId && hole.open(),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [service, paneId, parked, hole]);

  useEffect(() => () => release(service, paneId), [service, paneId]);

  // Sidebar toggles and page zoom move the slot; Rust follows window resizes
  // itself.
  useEffect(() => {
    const slot = slotRef.current;
    if (!slot) return;
    const observer = new ResizeObserver(() => {
      if (holders.get(service)?.paneId !== paneId) return;
      dock.setViewport(service, measure(slot)).catch(() => {});
    });
    observer.observe(slot);
    return () => observer.disconnect();
  }, [service, paneId]);

  return slotRef;
}
